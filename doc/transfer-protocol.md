Based on HTTP, but encryption (TLS 1.2 or better) is mandatory.

API URLs are versioned, `/v1/get_object`. There's also a `/protocol_versions`
endpoint which lists the supported versions.

There are no optional features in any version. If `v1` is supported, *all*
features must be supported in it. There are no extensions either.

Clients will first access the `/protocol_versions` endpoint for a server and
get back a newline delimited list of protocol versions. They can then pick the
highest one they are willing to work with.


# v1 

## `GET /object/by-id/<id>`

Retrieve an item by id.

The server does not have to check the hash of the object, as the client will do so. (And report it, if needed).

## `POST /object/bad-hash-report/<id>`

Requests that the server retrieve an object and then hash it to ensure it's
still good. Used to notify the server that the object it recently sent has a
bad hash. What the server decides to do if the hash fails is up to the server.

## `PUT /object/by-id/<id>`

MUST fail if the uploaded object, when hashed, does not match the id given.

## `GET /reflog/current/<id>`

Gets a single id from the server

## `GET /reflog/all/<id>`

Gets all reflogs from the server
