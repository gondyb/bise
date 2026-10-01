// Wire
// ====
// Base's socket effects decode and encode UTF-8. These move octets as they
// are: one Char per byte (0..255) in, and each Char's low byte out.

#ifndef WIRE_BYTES
#define WIRE_BYTES

static Term wire_bytes(Env e, const char* p, u64 n) {
  Term s    = term_pak(CID_SNIL, 0);
  u64  hole = 0;
  for (u64 i = 0; i < n; i += 1) {
    u64  l = heap_alloc(e, 1);
    Term t = term_ctr(CID_SCON, l);
    e.mem[l] = (uint8_t)p[i];
    if (hole == 0) {
      s = t;
    } else {
      e.mem[hole] = io_seal(e, t, CID_SCON);
    }
    hole = l + 1;
  }
  if (hole != 0) {
    e.mem[hole] = io_seal(e, term_pak(CID_SNIL, 0), CID_SCON);
  }
  return s;
}

// A Char above 255 is not an octet; the send fails with EINVAL.
static char* wire_octets(Env e, Term s, u64* len, bool* bad) {
  u64   cap = 64;
  u64   n   = 0;
  char* buf = io_mem(malloc(cap));
  *bad = false;
  while (term_aux(s) == CID_SCON) {
    Term fb[2];
    spare_free(e, cls_fit(2), ctr_take(e, s, 2, fb));
    if (n + 1 > cap) {
      cap *= 2;
      buf = io_mem(realloc(buf, cap));
    }
    *bad = *bad || (u64)fb[0] > 255;
    buf[n++] = (char)(fb[0] & 0xFF);
    s = fb[1];
  }
  *len = n;
  return buf;
}

// Bytes as bytes/bytes.bend packs them: byte i in bits 8*(i%4) of word i/4,
// 2^d words (the fewest that hold n), zero past n. Leans on the runtime's BUF
// block (blk_new, blk_loc, blk_read, blk_write); recheck on a Bend upgrade.
static Term wire_words(Env e, const char* p, u64 n) {
  u64  w    = (n + 3) / 4;
  u64 d    = 0;
  Term zero = 0;
  while ((1ull << d) < w) {
    d += 1;
  }
  Term a = blk_new(e, false, d, 0, 1, &zero);
  u64  l = blk_loc(e.mem, a);
  for (u64 k = 0; k < w; k += 1) {
    u32 x = 0;
    for (u64 j = 0; j < 4 && 4 * k + j < n; j += 1) {
      x |= (u32)(uint8_t)p[4 * k + j] << (8 * j);
    }
    blk_write(e.mem, false, l, (u32)k, x);
  }
  return io_tup(e, (Term)n, a);
}

// The first n bytes of a U32 BUF; n past its end fails with EINVAL.
static char* wire_words_octets(Env e, Term a, u64 n, u64* len, bool* bad) {
  u64* H = e.mem;
  *bad     = term_tag(a) != TAG_BUF || n > (4ull << blk_cls(a));
  *len     = *bad ? 0 : n;
  char* buf = io_mem(malloc(*len + 1));
  u64   l   = *bad ? 0 : blk_loc(H, a);
  for (u64 i = 0; i < *len; i += 1) {
    buf[i] = (char)(blk_read(H, false, l, (u32)(i / 4)) >> (8 * (i % 4)));
  }
  term_drop(e, a);
  return buf;
}

// A deadline in io_tick() nanoseconds, or 0 for none.
static u64 wire_deadline(u64 ms) {
  return ms != 0 ? io_tick() + ms * 1000000ull : 0;
}

static bool wire_late(u64 at) {
  return at != 0 && io_tick() >= at;
}

#endif

#if defined(CID(recv)) || defined(CID(recv.words))

// Not IO_READ: that would wait for readability before run, with no deadline.
// w->size holds the deadline until the read lands.
static Term wire_recv_go(Env e, IoWork* w, IoPack more, bool words) {
  int     fd = (int)w->hand;
  ssize_t n  = io_sys_end(w, recv(fd, w->data, (size_t)w->made, 0));
  if (w->code == EAGAIN) {
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, POLLIN, w->size, more);
    }
    w->code = ETIMEDOUT;
  }
  Term r = w->code ? io_fail(e, w->code, NULL)
    : io_done(e, words ? wire_words(e, w->data, (u64)n) : wire_bytes(e, w->data, (u64)n));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

static void wire_recv_init(Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->made = f[1] < INT32_MAX ? (intptr_t)f[1] : INT32_MAX;
  w->data = io_mem(malloc((size_t)w->made + 1));
  w->size = wire_deadline((u64)f[2]);
}

#endif

#ifdef CID(recv)

static Term wire_recv_more(Env e, IoWork* w) {
  return wire_recv_go(e, w, wire_recv_more, false);
}

Term wire_recv_run(Env e, Term* f, IoWork* w) {
  wire_recv_init(f, w);
  return wire_recv_more(e, w);
}

static void __attribute__((constructor)) wire_recv_use(void) {
  io_eff(CID(recv), wire_recv_run, 0);
}

#endif

#ifdef CID(recv.words)

static Term wire_recv_words_more(Env e, IoWork* w) {
  return wire_recv_go(e, w, wire_recv_words_more, true);
}

Term wire_recv_words_run(Env e, Term* f, IoWork* w) {
  wire_recv_init(f, w);
  return wire_recv_words_more(e, w);
}

static void __attribute__((constructor)) wire_recv_words_use(void) {
  io_eff(CID(recv.words), wire_recv_words_run, 0);
}

#endif

#if defined(CID(send)) || defined(CID(send.words)) || defined(CID(send.file))

static Term wire_send_more(Env e, IoWork* w) {
  int fd = (int)w->hand;
  while (w->code == 0 && (u64)w->made < w->size) {
    ssize_t n = send(fd, w->data + w->made, w->size - (u64)w->made, 0);
    if (n < 0 && errno == EAGAIN) {
      return io_wait_on(w, fd, POLLOUT, 0, wire_send_more);
    }
    w->made += io_sys_end(w, n);
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL)
    : io_done(e, term_pak(CID_UNIT, 0));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

#endif

#ifdef CID(send)

Term wire_send_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_octets(e, f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_send_more(e, w);
}

static void __attribute__((constructor)) wire_send_use(void) {
  io_eff(CID(send), wire_send_run, 0);
}

#endif

#ifdef CID(send.words)

Term wire_send_words_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_words_octets(e, f[2], (u64)f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_send_more(e, w);
}

static void __attribute__((constructor)) wire_send_words_use(void) {
  io_eff(CID(send.words), wire_send_words_run, 0);
}

#endif

#if defined(CID(send.file)) || defined(CID(tls.send.file))

// The file at path (octets, one Char per byte), exactly size bytes of it, in a
// malloc'd buffer: a body part read at send time never enters the Bend heap.
// A path that is not octets or a file of another size: EINVAL; else errno.
static char* wire_file(Env e, Term path, u64 size, u64* len, int* code) {
  bool  bad;
  u64   n;
  char* p   = wire_octets(e, path, &n, &bad);
  p         = io_mem(realloc(p, n + 1));
  p[n]      = 0;
  char* buf = io_mem(malloc(size > 0 ? size : 1));
  *code     = bad ? EINVAL : 0;
  *len      = size;
  if (*code == 0) {
    errno    = 0;
    FILE* fp = fopen(p, "rb");
    if (fp == NULL) {
      *code = errno != 0 ? errno : ENOENT;
    } else {
      u64 got = (u64)fread(buf, 1, size, fp);
      if (got != size || fgetc(fp) != EOF) {
        *code = EINVAL;
      }
      fclose(fp);
    }
  }
  free(p);
  return buf;
}

#endif

#ifdef CID(send.file)

Term wire_send_file_run(Env e, Term* f, IoWork* w) {
  int code;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_file(e, f[1], (u64)f[2], &w->size, &code);
  w->made = 0;
  w->code = code;
  return wire_send_more(e, w);
}

static void __attribute__((constructor)) wire_send_file_use(void) {
  io_eff(CID(send.file), wire_send_file_run, 0);
}

#endif

#ifdef CID(recv_from)

static Term wire_recv_from_more(Env e, IoWork* w) {
  struct sockaddr_in at = { 0 };
  socklen_t alen = sizeof(at);
  char      host[16];
  int       fd = (int)w->hand;
  ssize_t   n  = io_sys_end(w, recvfrom(fd, w->data, (size_t)w->made, 0,
    (struct sockaddr*)&at, &alen));
  if (w->code == EAGAIN) {
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, POLLIN, w->size, wire_recv_from_more);
    }
    w->code = ETIMEDOUT;
  }
  inet_ntop(AF_INET, &at.sin_addr, host, 16);
  Term r = w->code ? io_fail(e, w->code, NULL)
    : io_done(e, io_tup(e, io_str(e, host, strlen(host)),
      io_tup(e, ntohs(at.sin_port), wire_bytes(e, w->data, (u64)n))));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

Term wire_recv_from_run(Env e, Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->made = f[1] < INT32_MAX ? (intptr_t)f[1] : INT32_MAX;
  w->data = io_mem(malloc((size_t)w->made + 1));
  w->size = wire_deadline((u64)f[2]);
  return wire_recv_from_more(e, w);
}

static void __attribute__((constructor)) wire_recv_from_use(void) {
  io_eff(CID(recv_from), wire_recv_from_run, 0);
}

#endif

#ifdef CID(send_to)

static Term wire_send_to_more(Env e, IoWork* w) {
  struct sockaddr_in at;
  int     fd = (int)w->hand;
  ssize_t n  = -1;
  errno      = EINVAL;
  if (w->code == 0 && io_sys_addr(w->text, (u32)w->made, &at) == 0) {
    n = sendto(fd, w->data, w->size, 0, (struct sockaddr*)&at, sizeof(at));
  }
  io_sys_end(w, n);
  if (w->code == EAGAIN) {
    return io_wait_on(w, fd, POLLOUT, 0, wire_send_to_more);
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL)
    : io_done(e, term_pak(CID_UNIT, 0));
  free(w->text);
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

Term wire_send_to_run(Env e, Term* f, IoWork* w) {
  uint64_t hn = 0;
  bool     bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->text = io_cstr(e, f[1], &hn);
  w->made = (intptr_t)f[2];
  w->data = wire_octets(e, f[3], &w->size, &bad);
  w->code = bad || io_nul(w->text, hn) ? EINVAL : 0;
  return wire_send_to_more(e, w);
}

static void __attribute__((constructor)) wire_send_to_use(void) {
  io_eff(CID(send_to), wire_send_to_run, 0);
}

#endif

#ifdef CID(connect)

// Base's TCP.connect waits as long as the kernel does (~75 s); this one has a
// deadline. On each wake SO_ERROR reports a failure, and connect() again says
// EALREADY (still going) or EISCONN (done).
static Term wire_connect_end(Env e, IoWork* w, int err) {
  int fd = (int)w->hand;
  if (err != 0 && fd >= 0) {
    close(fd);
  }
  free(w->data);
  return err != 0 ? io_fail(e, (u32)err, NULL) : io_done(e, io_hand(fd));
}

static Term wire_connect_more(Env e, IoWork* w) {
  struct sockaddr_in at;
  int       fd  = (int)w->hand;
  int       err = 0;
  socklen_t len = sizeof(err);
  if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &len) != 0) {
    err = errno;
  }
  if (err == 0) {
    io_sys_addr(w->data, (u32)w->made, &at);
    err = connect(fd, (struct sockaddr*)&at, sizeof(at)) == 0 ? 0 : errno;
  }
  if (err == 0 || err == EISCONN) {
    return wire_connect_end(e, w, 0);
  }
  if (err == EINPROGRESS || err == EALREADY) {
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, POLLOUT, w->size, wire_connect_more);
    }
    err = ETIMEDOUT;
  }
  return wire_connect_end(e, w, err);
}

Term connect_run(Env e, Term* f, IoWork* w) {
  struct sockaddr_in at;
  u64 hn  = 0;
  w->data = io_cstr(e, f[0], &hn);
  w->made = (intptr_t)f[1];
  w->hand = -1;
  if (io_nul(w->data, hn) || io_sys_addr(w->data, (u32)w->made, &at) != 0) {
    return wire_connect_end(e, w, EINVAL);
  }
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd < 0) {
    return wire_connect_end(e, w, errno);
  }
  w->hand = fd;
  if (fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK) < 0) {
    return wire_connect_end(e, w, errno);
  }
  w->size = wire_deadline((u64)f[2]);
  return wire_connect_more(e, w);
}

static void __attribute__((constructor)) connect_use(void) {
  io_eff(CID(connect), connect_run, 0);
}

#endif

// TLS
// ===
// OpenSSL 3, loaded at run time (bend links no extra libraries). The SSL
// object of a socket lives in a table keyed by its fd. Peer verification
// (chain + host name) is always on; TLS 1.2 is the floor.

#if defined(CID(tls.connect)) || defined(CID(tls.send)) || defined(CID(tls.recv)) || defined(CID(tls.close)) \
  || defined(CID(tls.send.words)) || defined(CID(tls.recv.words))
#ifndef WIRE_TLS
#define WIRE_TLS
#include <dlfcn.h>

typedef struct {
  int   state;
  void* ctx;
  void* (*ssl_new)(void*);
  int   (*set_fd)(void*, int);
  long  (*ctrl)(void*, int, long, void*);
  int   (*set1_host)(void*, const char*);
  int   (*connect)(void*);
  int   (*read)(void*, void*, int);
  int   (*write)(void*, const void*, int);
  int   (*get_error)(const void*, int);
  int   (*shutdown)(void*);
  void  (*ssl_free)(void*);
  long  (*verify_result)(const void*);
  const char* (*verify_text)(long);
} WireTls;

#define WIRE_TLS_FDS 65536
static WireTls wire_tls;
static void*   wire_tls_ssl[WIRE_TLS_FDS];

static void* wire_tls_open(void) {
  const char* paths[] = { getenv("BEND_LIBSSL"),
    "/opt/homebrew/opt/openssl@3/lib/libssl.3.dylib",
    "/usr/local/opt/openssl@3/lib/libssl.3.dylib", "libssl.3.dylib", "libssl.so.3" };
  for (u64 i = 0; i < sizeof(paths) / sizeof(paths[0]); i += 1) {
    void* h = paths[i] != NULL ? dlopen(paths[i], RTLD_NOW | RTLD_LOCAL) : NULL;
    if (h != NULL) {
      return h;
    }
  }
  return NULL;
}

static bool wire_tls_load(void) {
  if (wire_tls.state != 0) {
    return wire_tls.state > 0;
  }
  wire_tls.state = -1;
  void* h = wire_tls_open();
  if (h == NULL) {
    return false;
  }
  void* (*method)(void)                 = dlsym(h, "TLS_client_method");
  void* (*ctx_new)(void*)               = dlsym(h, "SSL_CTX_new");
  int   (*paths)(void*)                 = dlsym(h, "SSL_CTX_set_default_verify_paths");
  void  (*verify)(void*, int, void*)    = dlsym(h, "SSL_CTX_set_verify");
  long  (*ctx_ctrl)(void*, int, long, void*) = dlsym(h, "SSL_CTX_ctrl");
  wire_tls.ssl_new       = dlsym(h, "SSL_new");
  wire_tls.set_fd        = dlsym(h, "SSL_set_fd");
  wire_tls.ctrl          = dlsym(h, "SSL_ctrl");
  wire_tls.set1_host     = dlsym(h, "SSL_set1_host");
  wire_tls.connect       = dlsym(h, "SSL_connect");
  wire_tls.read          = dlsym(h, "SSL_read");
  wire_tls.write         = dlsym(h, "SSL_write");
  wire_tls.get_error     = dlsym(h, "SSL_get_error");
  wire_tls.shutdown      = dlsym(h, "SSL_shutdown");
  wire_tls.ssl_free      = dlsym(h, "SSL_free");
  wire_tls.verify_result = dlsym(h, "SSL_get_verify_result");
  wire_tls.verify_text   = dlsym(h, "X509_verify_cert_error_string");
  if (!method || !ctx_new || !paths || !verify || !ctx_ctrl
    || !wire_tls.ssl_new || !wire_tls.set_fd || !wire_tls.ctrl
    || !wire_tls.set1_host || !wire_tls.connect || !wire_tls.read
    || !wire_tls.write || !wire_tls.get_error || !wire_tls.shutdown
    || !wire_tls.ssl_free || !wire_tls.verify_result || !wire_tls.verify_text) {
    return false;
  }
  void* ctx = ctx_new(method());
  if (ctx == NULL || paths(ctx) != 1) {
    return false;
  }
  verify(ctx, 1, NULL);              // SSL_VERIFY_PEER
  ctx_ctrl(ctx, 123, 0x0303, NULL);  // SSL_CTRL_SET_MIN_PROTO_VERSION, TLS 1.2
  // A bare EOF is an error. close_notify is the only clean close.
  wire_tls.ctx   = ctx;
  wire_tls.state = 1;
  return true;
}

static void* wire_tls_of(int fd) {
  return fd >= 0 && fd < WIRE_TLS_FDS ? wire_tls_ssl[fd] : NULL;
}

static void wire_tls_drop(int fd) {
  void* ssl = wire_tls_of(fd);
  if (ssl != NULL) {
    wire_tls.ssl_free(ssl);
    wire_tls_ssl[fd] = NULL;
  }
}

#endif
#endif

#ifdef CID(tls.connect)

static Term wire_tls_connect_end(Env e, IoWork* w, Term r) {
  free(w->text);
  return io_tup(e, io_hand(w->hand), r);
}

static Term wire_tls_connect_fail(Env e, IoWork* w, u32 code, const char* why) {
  wire_tls_drop((int)w->hand);
  return wire_tls_connect_end(e, w, io_fail(e, code, why));
}

static Term wire_tls_connect_more(Env e, IoWork* w) {
  int   fd  = (int)w->hand;
  void* ssl = wire_tls_of(fd);
  int   r   = wire_tls.connect(ssl);
  if (r == 1) {
    return wire_tls_connect_end(e, w, io_done(e, term_pak(CID_UNIT, 0)));
  }
  int err = wire_tls.get_error(ssl, r);
  if (err == 2 || err == 3) {  // SSL_ERROR_WANT_READ, SSL_ERROR_WANT_WRITE
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, err == 2 ? POLLIN : POLLOUT, w->size, wire_tls_connect_more);
    }
    return wire_tls_connect_fail(e, w, ETIMEDOUT, NULL);
  }
  long v = wire_tls.verify_result(ssl);
  return wire_tls_connect_fail(e, w, EPROTO, v != 0 ? wire_tls.verify_text(v) : "TLS handshake failed");
}

Term tls_connect_run(Env e, Term* f, IoWork* w) {
  uint64_t hn = 0;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->text = io_cstr(e, f[1], &hn);
  w->size = wire_deadline((u64)f[2]);
  int fd  = (int)w->hand;
  if (!wire_tls_load()) {
    return wire_tls_connect_end(e, w, io_fail(e, ENOENT, "TLS needs OpenSSL 3 (libssl.3); set BEND_LIBSSL to its path"));
  }
  if (fd < 0 || fd >= WIRE_TLS_FDS || io_nul(w->text, hn)) {
    return wire_tls_connect_end(e, w, io_fail(e, EINVAL, NULL));
  }
  void* ssl = wire_tls.ssl_new(wire_tls.ctx);
  if (ssl == NULL) {
    return wire_tls_connect_end(e, w, io_fail(e, ENOMEM, NULL));
  }
  wire_tls_ssl[fd] = ssl;
  // SNI (SSL_CTRL_SET_TLSEXT_HOSTNAME, host_name) and host name verification.
  if (wire_tls.set_fd(ssl, fd) != 1 || wire_tls.ctrl(ssl, 55, 0, w->text) != 1
    || wire_tls.set1_host(ssl, w->text) != 1) {
    return wire_tls_connect_fail(e, w, EPROTO, "TLS setup failed");
  }
  return wire_tls_connect_more(e, w);
}

static void __attribute__((constructor)) tls_connect_use(void) {
  io_eff(CID(tls.connect), tls_connect_run, 0);
}

#endif

#if defined(CID(tls.send)) || defined(CID(tls.send.words)) || defined(CID(tls.send.file))

// SSL_write is retried with the same buffer, as OpenSSL requires.
static Term wire_tls_send_more(Env e, IoWork* w) {
  int   fd  = (int)w->hand;
  void* ssl = wire_tls_of(fd);
  while (w->code == 0 && (u64)w->made < w->size) {
    if (ssl == NULL) {
      w->code = EBADF;
      break;
    }
    u64 left = w->size - (u64)w->made;
    int n    = wire_tls.write(ssl, w->data + w->made, left > INT32_MAX ? INT32_MAX : (int)left);
    if (n > 0) {
      w->made += n;
      continue;
    }
    int err = wire_tls.get_error(ssl, n);
    if (err == 2 || err == 3) {
      return io_wait_on(w, fd, err == 2 ? POLLIN : POLLOUT, 0, wire_tls_send_more);
    }
    w->code = EPIPE;
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL)
    : io_done(e, term_pak(CID_UNIT, 0));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

#endif

#ifdef CID(tls.send)

Term tls_send_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_octets(e, f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_tls_send_more(e, w);
}

static void __attribute__((constructor)) tls_send_use(void) {
  io_eff(CID(tls.send), tls_send_run, 0);
}

#endif

#ifdef CID(tls.send.words)

Term tls_send_words_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_words_octets(e, f[2], (u64)f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_tls_send_more(e, w);
}

static void __attribute__((constructor)) tls_send_words_use(void) {
  io_eff(CID(tls.send.words), tls_send_words_run, 0);
}

#endif

#ifdef CID(tls.send.file)

Term tls_send_file_run(Env e, Term* f, IoWork* w) {
  int code;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_file(e, f[1], (u64)f[2], &w->size, &code);
  w->made = 0;
  w->code = code;
  return wire_tls_send_more(e, w);
}

static void __attribute__((constructor)) tls_send_file_use(void) {
  io_eff(CID(tls.send.file), tls_send_file_run, 0);
}

#endif

#if defined(CID(tls.recv)) || defined(CID(tls.recv.words))

// Empty means close_notify. A bare EOF is a read error, not a clean close.
static Term wire_tls_recv_go(Env e, IoWork* w, IoPack more, bool words) {
  int   fd  = (int)w->hand;
  void* ssl = wire_tls_of(fd);
  int   n   = ssl != NULL ? wire_tls.read(ssl, w->data, (int)w->made) : -1;
  Term  r;
  if (n > 0) {
    r = io_done(e, words ? wire_words(e, w->data, (u64)n) : wire_bytes(e, w->data, (u64)n));
  } else {
    int err = ssl != NULL ? wire_tls.get_error(ssl, n) : 1;
    if (err == 2 || err == 3) {
      if (!wire_late(w->size)) {
        return io_wait_on(w, fd, err == 2 ? POLLIN : POLLOUT, w->size, more);
      }
      err = -1;
    }
    r = err == 6 ? io_done(e, words ? wire_words(e, w->data, 0) : term_pak(CID_SNIL, 0))  // SSL_ERROR_ZERO_RETURN
      : err == -1 ? io_fail(e, ETIMEDOUT, NULL)
      : io_fail(e, ssl != NULL ? EIO : EBADF, ssl != NULL ? "TLS read failed" : NULL);
  }
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

static void wire_tls_recv_init(Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->made = f[1] < INT32_MAX ? (intptr_t)f[1] : INT32_MAX;
  w->data = io_mem(malloc((size_t)w->made + 1));
  w->size = wire_deadline((u64)f[2]);
}

#endif

#ifdef CID(tls.recv)

static Term wire_tls_recv_more(Env e, IoWork* w) {
  return wire_tls_recv_go(e, w, wire_tls_recv_more, false);
}

Term tls_recv_run(Env e, Term* f, IoWork* w) {
  wire_tls_recv_init(f, w);
  return wire_tls_recv_more(e, w);
}

static void __attribute__((constructor)) tls_recv_use(void) {
  io_eff(CID(tls.recv), tls_recv_run, 0);
}

#endif

#ifdef CID(tls.recv.words)

static Term wire_tls_recv_words_more(Env e, IoWork* w) {
  return wire_tls_recv_go(e, w, wire_tls_recv_words_more, true);
}

Term tls_recv_words_run(Env e, Term* f, IoWork* w) {
  wire_tls_recv_init(f, w);
  return wire_tls_recv_words_more(e, w);
}

static void __attribute__((constructor)) tls_recv_words_use(void) {
  io_eff(CID(tls.recv.words), tls_recv_words_run, 0);
}

#endif

#ifdef CID(tls.close)

// ponytail: one non-blocking close_notify attempt; no wait for the peer's
Term tls_close_run(Env e, Term* f, IoWork* w) {
  int   fd  = (int)io_hand_v(f[0]);
  void* ssl = wire_tls_of(fd);
  if (ssl != NULL) {
    wire_tls.shutdown(ssl);
    wire_tls_drop(fd);
  }
  close(fd);
  return term_pak(CID_UNIT, 0);
}

static void __attribute__((constructor)) tls_close_use(void) {
  io_eff(CID(tls.close), tls_close_run, 0);
}

#endif
