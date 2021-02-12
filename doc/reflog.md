# reflog

The reflog is how branch names are communicated.

Below is a sketch of what it looks like.

```rust
struct Reflog {
    packets: Vec<Packet>,
}

struct Packet {
    updates: Vec<Update>,
}

struct Update {
    branch_name: String,

    // None means "delete"
    target_hash: Option<Key>,
}
```

The reflog is generally append only, but it would make sense to allow squashes
of the log to hide sensitive information from persisting in history. The server
can tell the difference between a squash and an append, so it can permission
them differently.

The client can request a range of packets from the server. Assuming there is no
squash, Reflog[0] will always refer to the same object.

If the client requests a range that contains the last item, they are given a
token. This token must be handed back to the server when appending an item to
the reflog. If someone else pushed before you, it will fail. (Similar to git's
`--force-with-lease`, but using an arbitrary token instead of a commit id).

This forces the client to acknowledge all packets before they can make a push.

This token must be invalidated in the event of a squash. The token being valid
means that no pushes or squashes have taken place, that the reflog on-server is
identical to what the client expects it to be.
