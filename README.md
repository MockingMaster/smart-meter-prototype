# Smart Meter Prototype

## Server Setup
The server might work on windows, but it has not been tested.
### HAProxy
The server requires HAProxy to handle TLS, this can be installed using a package manager on most distros:  
Debian based: apt install haproxy  

Certificates/haproxy.pem needs to be moved to `/etc/ssl/certs/haproxy.pem`

Start haproxy service:

```
service haproxy start
```

### Server
```
cargo run [--release]
```
####  Tests
```
cargo test
```
