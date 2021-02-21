# Goals

All of these hold only when the server does not know the key.

If a client inserts an attacker controlled/known file into the repository and
pushes it to an attacker controlled server, the attacker has no way of telling
the difference between the client inserting that file, and inserting any other
file.

The server is unable to read any client data (objects or reflog entries).

The server is unable to change any client data without detection. Any attempt
to do so will result in an error when the data is read. The server rolling back
data to a previously valid state *is* possible.


# Crypto

Servers should not be able to know what data they are storing (even if the data is controlled by the server).

Each repository has a 32 byte repo key, which is used to encrypt all data. This
is mandatory. Repos that are public can use a key of all zeros. This is done to
ensure that the cryptography code is always run, instead of being an edge case
that no one cares about.

Data can be deduplicated amongst repos that have the same key, therefore using
encryption does not harm deduplication.

From the repo key, we use blake3 derive_key with a context of "snapcd <commit
timestamp of implementation> encryption key" as well as the same but with
"reflog encryption key key".

Let these be EKey and RLKey respectively.

We also need a 64 byte*256 table for gearhash. Derive this.

# From file to server

Let's assume you already have the repo key (RKey) and want to upload a file.

Derive your custom table for gearhash, and chunk the file into chunks with it.

Take the chunk that you have, and encrypt it with AES-GCM-SIV with a **static
nonce of all zeros** using EKey. (See below why I do this!)

You know have an object that can be hashed with unkeyed Blake3 to make up the
identifier.

This object is sent to the server (along with the hash) to be stored.

# and back...

Let's say we want to download a chunk from the server, where we know the hash.

Well, we just ask the server for the file with said hash.

Once we get it, we can decrypt it with our EKey. If there's a failure here,
then the server has tampered with the file, or it got corrupted.

After that, we must then hash the *ciphertext* to ensure that the server sent
us the correct object. If *this* fails, then the server sent us an object
created by someone who knows the key, but it wasn't the object we were
expecting.

TODO: Is the order here particularly important?

Finally, after both checks pass, we can consider the object valid.

# reflog

Unlike in git, the mapping from branch name to commit id is done through an
actual log.

The details of this are detailed in reflog.md, but here I'll spec out the
crypto here.

It's an append only log of packets. Each packet is encrypted similary to above,
using the same algorithm. However, it uses RLKey instead of re-using EKey,
*and* a random nonce is used rather than using static all zeros. This is
because we *do* need the property that you can't check a guess at the
plaintext, as branch names may be predictable, and will refer to known hashes.

Reordering/dropping of reflog packets is prevented by the hash chain in the
reflog packet. The server is able to drop reflog packets from the head of the
log. Clients should show a fatal error and refuse to continue if they detect
this.

# So about that static nonce?

https://cyber.biu.ac.il/aes-gcm-siv/ claims that a static nonce is safe up to
2^48 blocks. Objects ~~are~~ *will be* limited to 64KiB (2^12 128 bit blocks) so that would
lead to a maximum object count of 48-12= 2^36 maximum objects to be encrypted
under a key. So I believe this is fine actually :tm:.

This system originally said that the object hash was a keyed hash of the
plaintext, and that the server just had to trust that whatever the client sent
over is correct. I originally thought this was not really a problem, but
[/u/Natanael_L over on
/r/crypto](https://www.reddit.com/r/crypto/comments/liyxhr/how_dangerous_is_setting_the_nonce_to_be_the_hash/gn7n8tt/)
pointed out that this allows anyone with the
key (which is everyone in the event of public repos) can send along incorrect
data for common keys, and the server has no way to verify it, thereby
corrupting the repo until someone can fix it and upload the correct object for
that key.

This breaks the assumption that 2 objects with the same hash can always be
treated identically, and is a big problem. So the objects are identified by
their ciphertext, and there is a deterministic mapping from plaintext object to
ciphertext using the key.
