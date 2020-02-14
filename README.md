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
implemented as a SQLite database with a few tables. Other data storage methods are easy to add,
and there is a somewhat functional [sled](https://github.com/spacejam/sled) backend.

Every object stored is CBOR encoded with 3 fields, `data`, for a series of bytes, `keys`, for a
series of keys that this object depends on, and `objtype`, a string to identify the type of the
object. `keys` is separate from data so a garbage collector doesn't need to understand every object
type in order to know what it can depend on. The order of keys in `keys` can be depended on to be
stable.

The encoded object is then hashed with Blake3B, and put into the database.

### Object Types

A `file.blob` is an arbitrary chunk of bytes. Usually around 8k long in the current implementation.

A `file.blobtree` is an object that points to a number of either blobs, or blobtrees. (At the moment, it
will definitely point to only one of those types, but this may not be guaranteed in the future. The
reader supports mixed blobs and blobtrees as children of a blobtree.

A `dir.FSItem.file` is a file. It gives a name and size to a blob.

A `dir.FSItem.dir` is a directory. It has files and directories. Children names are stored in the
data section (CBOR encoded, along with other metadata), and they directly correspond to child keys.

A `commit.commit` is a commit.

It has a list of parents (Can be empty for a root), a tree, and a HashMap<String, String> for misc
attributes. Like commit message. Or author name. At the moment this is always empty

## oh cool how do i use this

`cargo run` will print help.

`cargo run init` will initalise the database in the current directory (much like `git init`). It
can be found in `.snapcd/snapcd.db`.

`cargo run insert <path>` is how you insert a path. It will print out a key for the item it just entered.

You can then fetch the path with `cargo run fetch <key> <dest>`. The key is allowed to be truncated
(actually there's a bug when you enter full keys and it doesn't work so truncate it to say 10
characters or something (this bug still exists please be patient keep truncating)).

It's *probably* safe to run, I made a best effort to not overwrite your data. Still, generally,
don't run untrusted code, and you shouldn't trust me to write good code. The code that actually
interacts with a file system is in `dir.rs`, and we refuse to overwrite existing files when
extracting.

For inspecting the internals, `cargo run debug pretty-print <key>` will print out the type, all the
keys that are linked to, the raw data (in hex), and a deserialised form of the data if possible
(because `data` *should* be CBOR for most cases, but that's not guaranteed, in the case of blobs
it's not true).

If you want to run `checkout`, remove the asserts I put in `dir::checkout_fs_item`. I'm a shit
coder, and don't trust myself to not do something stupid.

## how quick is it?

If you want to backup your data to be safe and sound in volatile memory, it's nice and speedy at
27 microseconds for 32 bytes of data (this benchmark is more looking at the time rather than
throughput), and 7.8ms for 4MB of data, which gives you 508MiB/s! This is all on my machine. (Ryzen
7 1700, all in a single thread)

You want to *save* your data? To disk? You're gonna get 60MiB/s, as measured by wall clock time to
insert 500MiB being 8 seconds. I'm blaming SQLite for this, I can print the hash very quickly but
the actual commit is rather slow. Not bad, but not great either.  Maybe an alternate data store can
make this better. Or maybe striping to multiple databases with multiple threads? My performance
goal is to not be the bottleneck, ever, even on low powered devices.
