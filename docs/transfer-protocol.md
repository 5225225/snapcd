Based on HTTP, but encryption (TLS 1.2 or better) is mandatory.

Client 

API URLs are versioned, `/v1/get_object`. There's also a `/protocol_versions`
endpoint which lists the supported versions.

There are no optional features in any version. If `v1` is supported, *all*
features must be supported in it. There are no extensions either.

Clients will first access the `/protocol_versions` endpoint for a server and
get back a newline delimited list of protocol versions. They can then pick the
highest one they are willing to work with.

All of these URLs are appended to a base URL. This base URL is how the server
knows what repository you're talking about.

# v1 

## `GET /object/by-id/<id>`

Retrieve an item by id.

## `PUT /object/by-id/<id>`

Uploads an object by id.

## `GET /reflog/by-index/<index>`

Gets a single reflog item from the server.

## `POST /reflog/append/<token>`

Appends an item to the reflog using this token.

May fail even if the reflog has not been modified (In the event of a server
reboot, for example). In that case, clients should get a new token and try
again.
