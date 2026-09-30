# Byte-exact TCP, UDP, and TLS sockets.
import Base

# Socket IO on octets: one Char per byte (0..255). Base's TCP/UDP effects
# decode UTF-8, which breaks byte counts and binary protocols.
# ms arguments are deadlines in milliseconds (0: none); a miss is ETIMEDOUT.
#   import ./wire.bend as Wire

# TCP connect to an IPv4 address, with a deadline.
def connect(host: String, port: U32, ms: U32) ->
  IO(Result<&1, &1, U32 & String, Socket>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def recv(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, String>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def send(sock: Socket, data: String) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def recv_from(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, String & U32 & String>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def send_to(sock: Socket, host: String, port: U32, data: String) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# TLS over a connected TCP socket (OpenSSL 3, loaded at run time; BEND_LIBSSL
# overrides its path). The certificate chain and host name are always checked.
def tls.connect(sock: Socket, host: String, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def tls.send(sock: Socket, data: String) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# "" once the peer sends close_notify. A bare EOF is an error.
def tls.recv(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, String>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# Ends the TLS session and closes the socket.
def tls.close(sock: Socket) -> IO(Unit):
  import "./effs/wire.c"
  import "./effs/wire.js"

# The .words forms move (len, words): bytes packed four to a U32, laid out as
# bytes/bytes.bend packs them, so no String cell is made per byte.
def recv.words(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# len past the end of words fails with EINVAL.
def send.words(sock: Socket, len: U32, words: Array<U32>) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# len 0 once the peer sends close_notify.
def tls.recv.words(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def tls.send.words(sock: Socket, len: U32, words: Array<U32>) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"
