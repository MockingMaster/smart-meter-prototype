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



## Client Setup

Requires latest version of Python3.

pip install:
customtkinter  
socket  
json  
time  
random  
threading  
ssl  
multiprocessing  
datetime  
struct  
queue  

Numer of clients adjustable in "__main__".

