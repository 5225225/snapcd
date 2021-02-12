# Crypto

Servers should not be able to know what data they are storing.

Each repository has a 32 byte repo key, which is used to encrypt all data. This
is mandatory. Repos that are public can use a key of all zeros. This is done to
ensure that the cryptography code is always run, instead of being an edge case
that no one cares about.

Data can be deduplicated amongst repos that have the same key, therefore using
encryption does not harm deduplication.

From the repo key, we use blake3 derive_key with a context of "snapcd <commit
timestamp of implementation> encryption key" as well as the same but with
"identification key".

Let these be EKey and IKey respectively.

We also need a 64 byte*256 table for gearhash. Derive this.

> This helps protect against fingerprinting attacks. Though how serious this is
> an issue I can't see.
> 
> Maybe padding would be a better move.

# From file to server

Let's assume you already have the repo key (RKey) and want to upload a file.

Derive your custom table for gearhash, and chunk the file into chunks with it.

Take the chunk you have, and hash it with a keyed blake3 using IKey.

> This gives you a hash that is unique for all (IKey, chunk data) tuples, and
> means that an attacking server can't prove that you have a particular file by
> looking at the hash it gets, unless it has RKey (in which case it can just
> decrypt your files)

> The server is unable to check integrity of the objects it holds purely off
> the hash that it needs to store, so it should store its own. This is outside
> the scope of this document.

> I considered encrypting the file and then doing the hash based off that, but
> that *requires* deterministic encryption. This scheme does not, but if you go
> to upload a chunk to the server that the server already has, *strictly
> speaking* the encrypted data will be different. However, duplicates can still
> be detected. This only affects the encryption layer, as two pieces of the
> same data can be treated identically.

You now have a hash for this object. This hash is what's public, and is sent to
the server, as well as being stored permanently in your objects.

Use a libsodium secretbox (xsalsa20 poly1305) and use your EKey to encrypt the
data with a random nonce.

Now that it's encrypted, we can send this (along with the hash!) to the server.

# and back...

Let's say we want to download a chunk from the server, where we know the hash.

Well, we just ask the server for the file with said hash.

Once we get that, we use our EKey to decrypt it. Any server tampering or
corruption will be detected here.

Once we get that, we might as well verify that the data is what we expect
(Maybe there's a buggy client?). Take your blob of data, and hash it with a
keyed blake3 just like you did to generate the hash in the first place.

If there's a mismatch *here*, then you have a bad client.
