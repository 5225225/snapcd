## What is this?

Directory archiving / sync / backup tool.

Design is somewhat similar to git, with the idea for chunked storage taken from
content defined chunking backup tools like borg and restic. (The idea for a tree of chunks was
taken from bup)

## Design goals

1. Data robustness.

    It should never lose your data. Even when it doesn't work.

    Good checks for corrupt repos are important here. We don't have a fsck at the moment, but it
    would not be hard to write one that iterates through all objects, checking that they're
    well-formed CBOR and point to valid keys.

2. Reliability

    It should always work. When it doesn't, see point 1.

3. Long-lasting

    Have an upgrade path in place for anything that might need upgrading. (See: hash-encoding.txt)


## Internals

Much like git, the core data store is currently just a content-addressable store. Currently, that's
implemented as a SQLite database with a single table.

Every object stored is CBOR encoded with 3 fields, `data`, for a series of bytes, `keys`, for a
series of keys that this object depends on, and `objtype`, a string to identify the type of the
object. `keys` is separate from data so a garbage collector doesn't need to understand every object
type in order to know what it can depend on. The order of keys in `keys` can be depended on to be
stable.

The encoded object is then hashed with Blake2B, and put into the database.

### Object Types

A `file.blob` is an arbitrary chunk of bytes. Usually around 8k long in the current implementation.

A `file.blobtree` is an object that points to a number of either blobs, or blobtrees. (At the moment, it
will definitely point to only one of those types, but this may not be guaranteed in the future. The
reader supports mixed blobs and blobtrees as children of a blobtree.

A `dir.FSItem.file` is a file. It gives a name and size to a blob.

A `dir.FSItem.dir` is a directory. It has files and directories.

Note how the file name is against the file, and not stored when the directory is linking to a file.

I'm not sure how I feel about this. Maybe it's better?

And finally...

A `commit.commit` is a commit.

It has a list of parents (Can be empty for a root), a tree, and a HashMap<String, String> for misc
attributes. Like commit message. Or author name.

## oh cool how do i use this

`cargo run` will print help.

`cargo run insert <path>` is how you insert a path. By default, the database is `snapcd.db`, a
sqlite3 database in the current directory (So if you move, snapcd won't find the database!). It
will print out a key for the item it just entered.

You can then fetch the path with `cargo run fetch <key> <dest>`. The key is allowed to be truncated
(actually there's a bug when you enter full keys and it doesn't work so truncate it to say 10
characters or something).

It's *probably* safe to run, I made a best effort to not overwrite your data. Still, generally,
don't run untrusted code, and you shouldn't trust me to write good code. The code that actually
interacts with a file system is in `dir.rs`, and we refuse to overwrite existing files when
extracting.

For inspecting the internals, `cargo run debug pretty-print <key>` will print out the type, all the
keys that are linked to, the raw data (in hex), and a deserialised form of the data if possible
(because `data` *should* be CBOR for most cases, but that's not guaranteed, in the case of blobs
it's not true).

Also add `--release` for those if you don't want it taking forever. Inserting is *slow* (1MB/s on
my machine, optimal is around 50 MB/s) in debug mode, I rely heavily on the optimiser to unfuck my code.
