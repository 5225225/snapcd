# Commit Display

I'd like a unified way to talk about how to view a specific commit.

## The commit data itself

A commit is just

```rust
struct Commit {
    tree: Key,         // FSItemDir
    parents: Vec<Key>, // Commit
    attrs: CommitAttrs,
}

pub struct CommitAttrs {
    pub message: String,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_cbor::Value>,
}
```

Where additional information may be added to `CommitAttrs` in future.

Here, the *only* information you have to display that is relevant in isolation
is `CommitAttrs` (and the commit hash itself).

Git log looks like

```
commit 559e90325fc96b4180be003147b1d2da1be4fd8c (HEAD -> main, origin/main)
Author: 5225225 <5225225@mailbox.org>
Date:   Thu Apr 29 00:57:59 2021 +0100

    Librarify the chunker a bit.
```

and 

```
commit 9529978f319ff13cd6c334a96fc41040ae573f34
Merge: 264a9d6 c69cafe
Author: 5225225 <5225225@mailbox.org>
Date:   Sun Apr 25 17:39:23 2021 +0100

    Merge branch 'cap-std'
```

Most important is the commit hash. No matter what, if you do a `log` or `show`
on a commit, it *must* show at least enough of the hash to be unambiguous given
the current database state (and add a few characters on for good measure).

## Diff

For now, I'm only considering diffing between a commit and *up to one* parent.
The commit being diffed must exist, but the parent is an `Option<Commit>`, to
handle the case where you do `snapcd show <initial commit>`, or `log` of that
initial commit.

Options are

1. Full. Basically whatever `git show` does.

2. Filenames. Basically whatever `snapcd status` does. Just shows added/deleted/modified, (and ideally line/byte count changes).

3. None.
