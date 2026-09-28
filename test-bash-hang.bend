# test-bash-hang.bend — the bash wrapper never hangs when it cannot
# write its own files (a full disk, a read-only or missing bg dir).
#
# Once: on a full disk the runner could not write <id>.rc, and the
# wrapper's wait loops (waiting for a .pid or a .rc file) spun forever;
# a live bash call hung 19 minutes. The disk-full errors are simulated
# here with permissions (chmod a-w) and with a directory squatting a
# slot file's name: the same "cannot write the file" failure, no full
# disk needed.
#
#   bend test-bash-hang.bend
#
# Each wrapper runs under `perl -e 'alarm 10; exec ...'`: a wrapper
# still running after 10 s is killed by SIGALRM (exit 142) and the case
# FAILs as a hang, so the suite itself always ends. Gate: no FAIL line,
# and the summary line is printed.
import Base
import runtime/bash-pure.bend as Shp
import runtime/proc.bend as Proc
import runtime/persist.bend as P

def root() -> String:
  "/tmp/bend-bash-hang-test"

def check(cond: Bool, +name: String, +out: String) -> IO(Unit):
  match cond:
    case True{}:
      do IO<Unit>:
        u : Unit <- IO.print(String.append("PASS ", String.append(name, "
")))
        IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        u : Unit <- IO.print(String.append("FAIL ", String.append(name,
          String.append("
  got: ", String.append(out, "
")))))
        IO.pure(Unit, Unit{})

def sh(+cmd: String) -> IO(String):
  Proc.run(Con{"/bin/sh", Con{"-c", Con{cmd, Nil{}}}})

# write the wrapper for <command> on <bg>, run it under a 10 s alarm;
# the answer is Proc.run's: the exit status line, then the output
def run_wrapped(+name: String, +command: String, +bg: String) -> IO(String):
  do IO<String>:
    +path : String <- IO.pure(String,
      String.append(root(), String.append("/wrap-", String.append(name, ".sh"))))
    w : Unit <- P.write_file(path, Shp.bg_wrap(command, bg, 5))
    Proc.run(Con{"perl", Con{"-e", Con{"alarm 10; exec @ARGV",
      Con{"/bin/sh", Con{path, Nil{}}}}}})

def not_hung(+out: String) -> Bool:
  Bool.not(String.starts_with(out, "142
"))

def main() -> IO(Unit):
  do IO<Unit>:
    +r : String <- IO.pure(String, root())
    s0 : String <- sh(String.append("chmod -R u+w ", String.append(r,
      String.append(" 2>/dev/null; rm -rf ", String.append(r,
        String.append("; mkdir -p ", String.append(r,
          String.append("/ro ", String.append(r,
            String.append("/rodir && chmod a-w ", String.append(r,
              String.append("/ro ", String.append(r, "/rodir")))))))))))))

    # ---- h1 the bg dir is missing and cannot be created ----
    +o1 : String <- run_wrapped("h1", "echo never",
      String.append(r, "/ro/bend-bg-0"))
    u1 : Unit <- check(Bool.and(not_hung(o1),
        Bool.and(String.contains(o1, "cannot create a slot"),
          Bool.not(String.contains(o1, "never")))),
      "h1 missing, uncreatable bg dir fails fast", o1)

    # ---- h2 the bg dir exists but is read-only ----
    +o2 : String <- run_wrapped("h2", "echo never",
      String.append(r, "/rodir"))
    u2 : Unit <- check(Bool.and(not_hung(o2),
        String.contains(o2, "cannot create a slot")),
      "h2 read-only bg dir fails fast", o2)

    # ---- h3 the .rc cannot be written (the live 19-minute hang) ----
    # the command makes its own bg dir read-only: the runner's rc write
    # (and its pid cleanup) fail after the command ended
    +b3 : String <- IO.pure(String, String.append(r, "/bend-bg-3"))
    +o3 : String <- run_wrapped("h3",
      String.append("chmod a-w ", String.append(b3, "; echo rc-probe")), b3)
    u3 : Unit <- check(Bool.and(not_hung(o3),
        Bool.and(String.starts_with(o3, "0
"),
          Bool.and(String.contains(o3, "rc-probe"),
            String.contains(o3, String.append("could not write ",
              Shp.bg_slot(b3, "0", ".rc")))))),
      "h3 unwritable .rc returns with the output and a note", o3)

    # ---- h4 the .pid cannot be written ----
    +b4 : String <- IO.pure(String, String.append(r, "/bend-bg-4"))
    s4 : String <- sh(String.append("mkdir -p ",
      Shp.bg_slot(b4, "0", ".pid")))
    +o4 : String <- run_wrapped("h4", "echo pid-probe; exit 3", b4)
    u4 : Unit <- check(Bool.and(not_hung(o4),
        Bool.and(String.starts_with(o4, "3
"), String.contains(o4, "pid-probe"))),
      "h4 unwritable .pid still returns the output and the exit code", o4)

    # ---- h5 the .out cannot be written ----
    +b5 : String <- IO.pure(String, String.append(r, "/bend-bg-5"))
    s5 : String <- sh(String.append("mkdir -p ",
      Shp.bg_slot(b5, "0", ".out")))
    +o5 : String <- run_wrapped("h5", "echo out-probe", b5)
    u5 : Unit <- check(Bool.and(not_hung(o5),
        String.contains(o5, String.append("could not write ",
          Shp.bg_slot(b5, "0", ".out")))),
      "h5 unwritable .out is reported, never hangs", o5)

    # ---- h6 the ordinary path is unchanged ----
    +b6 : String <- IO.pure(String, String.append(r, "/bend-bg-6"))
    +o6 : String <- run_wrapped("h6", "echo plain; exit 4", b6)
    u6 : Unit <- check(String.eq(o6, "4
plain
"), "h6 plain command: exact output and exit code", o6)

    sz : String <- sh(String.append("chmod -R u+w ", String.append(r,
      String.append(" 2>/dev/null; rm -rf ", r))))
    fin : Unit <- IO.print("---- bash-hang suite done
")
    IO.pure(Unit, Unit{})
