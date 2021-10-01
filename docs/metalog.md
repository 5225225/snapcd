# MetaLog(s)

The problem is that we want the ability to store metadata (branch mappings),
and ideally protect branches to being *modified*, even to people who have the
key.

Each device has an asymmetric key. This is somewhat like a SSH key, in the
sense that you're not expected for it to leave a device, you simply generate
multiple.

All messages must be signed by a key. Invalid messages (ones signed by an invalid *at the time* key) are ignored.
Servers should take steps to avoid flooding of invalid data, such as ratelimits.

## Permissions

Each key has a permission state that it has, which looks like

```rust
struct PermissionState {
    root: bool,
    branch_permissions: HashMap<String, BranchPermissions>,
}

struct BranchPermissions {
    push: bool,
    delete: bool,
}
```

`root`: Bypass *all* permission checks. A message signed with a valid key with the `root` permission always succeeds.

## Conflicts

Each message can have a "conflict marker" associated with it on the server side. This is temporary.

Each message has an index associated with it. You don't know the index of any
new messages you're inserting until you fetch and see them in the metalog, but
you *do* know the index of any messages you've fetched.

You send along how many messages you've seen. Any messages that have been
inserted afterwards, with any conflict markers that match any of the ones
you're putting on your message, will block the message from being inserted, and
return an error.

For example, two clients both see

```
0: Init
1: UpdateRef { name: "main", value: A } // conflicts: main
```

Say we wanted to insert

```
UpdateRef { name: "main", value: B } // conflicts: main
```

But someone else inserted

```
UpdateRef { name: "main", value: C } // conflicts: main
```

We want this to fail. This inserting won't cause *harm* to the repository, as
update references only work if the old value is what was expected.

But it's still a waste of bandwidth.

So here, once the UpdateRef to hash C was inserted, *all* clients pushing
messages with a conflicts of `main` will get an error.

Note that conflicts are server-side known. So hashing them is a good idea. You
can even hash them and only use the first byte of the hash, since the conflicts
feature is only an optimisation, all messages must be expected to work in the
event that they're raced with.

## Messages

There are multiple message types that can happen.

### Init

```rust
struct InitMessage;
```

This message is mandatory, and can only happen once, as the very first message. The signer of this message is granted `root` permission.

### Update Reference

```rust
struct UpdateRef {
    name: String,
    old_value: Option<ObjectHash>,
    value: Option<ObjectHash>,
}
```

The existence of the reference denoted by the name is denoted by `value`. If it
is Some(key), then the reference is either created or updated to be that key.
If it is None, the reference is deleted.

If the old value did not match the `value` of the most recent call to `UpdateRef` for the specified `name`, then the update is ignored.

When a reference is deleted, all permissions no longer apply to it if it was recreated.

## Rewrites

The metalog can get long, and may contain historical personal data that you would like to remove.

Rewrites are handled by locking the main 
