# core/patch.bend — the apply_patch tool's pure core, in the Codex V4A
# format.
#
#   *** Begin Patch
#   *** Add File: <path>
#   +line
#   *** Update File: <path>
#   @@ optional context hint (ignored by the matcher, kept for the model)
#    context line
#   -removed line
#   +added line
#   *** Delete File: <path>
#   *** End Patch
#
# Parsing is ONE pass over KIND-ANNOTATED lines: p_kind_of classifies
# each line once, and every loop then dispatches on the kind alone (a
# computed scrutinee is never legal in this repo - pre-annotating the
# lines is what keeps the parser free of routing cycles).
#
# The UPDATE matcher is STRICT and sequential: each chunk's
# context+removed lines must occur in the file, in order, starting
# after the previous chunk's splice - a miss fails LOUDLY with the
# chunk number ("the file changed since you read it, re-read and
# adjust" is the ergonomics the model needs; a fuzzy guesser silently
# corrupts code).
#
# Codex-parity decisions: the whole patch is trimmed; the first line
# must be '*** Begin Patch' and the last '*** End Patch' (loud on
# both); update content may open a chunk without @@; a bare empty
# line is an empty context line; an empty chunk (a lone @@) is
# skipped; after End Patch only empty lines are tolerated. NOT
# supported yet, and rejected loudly as garbage: *** Move to:,
# *** End of File, *** Environment ID.
#
# Two live-model tolerances (observed end-to-end with GLM, pinned by
# the laws): the Begin/End markers match by PREFIX (the model appends
# stray ' ***'), and section headers parse WITHOUT the leading '***'
# too - safe, because a real content line inside a chunk always
# carries a ' ', '+' or '-' prefix, so a bare 'Update File:' line can
# only be a header.
import Base

# ---- the line vocabulary ----

def p_nl() -> Char:
  Char.from_u32(10)

# one annotated line: the raw text plus its kind (classified once)
type PKind is Data:
  KAdd{}          # "+x"   an added line
  KDel{}          # "-x"   a removed line
  KCtx{}          # " x"   a context line
  KEmpty{}        # ""     a bare empty line (empty context in updates)
  KBegin{}        # "*** Begin Patch" (trimmed compare)
  KEnd{}          # "*** End Patch" (trimmed compare)
  KAddHdr{}       # "*** Add File: <path>"
  KDelHdr{}       # "*** Delete File: <path>"
  KUpdHdr{}       # "*** Update File: <path>"
  KHead{}         # "@@..." a chunk header
  KGarbage{}      # anything else

type PL is Data:
  PL{line: String, kind: PKind}

# ---- the actions ----

type PChunk is Data:
  PChunk{old: List<&2, String>, new: List<&2, String>, olen: Nat, nlen: Nat}

type PAction is Data:
  PAdd{path: String, content: String}
  PDelete{path: String}
  PUpdate{path: String, chunks: List<&2, PChunk>}

# ---- the classifier (a backward chain: one def per prefix) ----

def p_kind.head(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KHead{}
    case False{}:
      KGarbage{}


def p_kind.updhdr_bare(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KUpdHdr{}
    case False{}:
      p_kind.head(String.starts_with(l, "@@"), l)



def p_kind.updhdr(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KUpdHdr{}
    case False{}:
      p_kind.updhdr_bare(String.starts_with(l, "Update File: "), l)


def p_kind.delhdr_bare(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KDelHdr{}
    case False{}:
      p_kind.updhdr(String.starts_with(l, "*** Update File: "), l)



def p_kind.delhdr(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KDelHdr{}
    case False{}:
      p_kind.delhdr_bare(String.starts_with(l, "Delete File: "), l)


def p_kind.addhdr_bare(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KAddHdr{}
    case False{}:
      p_kind.delhdr(String.starts_with(l, "*** Delete File: "), l)



def p_kind.addhdr(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KAddHdr{}
    case False{}:
      p_kind.addhdr_bare(String.starts_with(l, "Add File: "), l)


def p_kind.end(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KEnd{}
    case False{}:
      p_kind.addhdr(String.starts_with(l, "*** Add File: "), l)



def p_kind.begin(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KBegin{}
    case False{}:
      p_kind.end(String.starts_with(String.trim(l), "*** End Patch"), l)



def p_kind.empty(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KEmpty{}
    case False{}:
      p_kind.begin(String.starts_with(String.trim(l), "*** Begin Patch"), l)



def p_kind.ctx(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KCtx{}
    case False{}:
      p_kind.empty(String.eq(l, ""), l)



def p_kind.del(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KDel{}
    case False{}:
      p_kind.ctx(String.starts_with(l, " "), l)



def p_kind.add(b: Bool, +l: String) -> PKind:
  match b:
    case True{}:
      KAdd{}
    case False{}:
      p_kind.del(String.starts_with(l, "-"), l)

def p_kind_of(+l: String) -> PKind:
  p_kind.add(String.starts_with(l, "+"), l)

# ---- the annotated-line map ----

def p_pls.go(lines: List<&2, String>, +acc: List<&2, PL>) -> List<&2, PL>:
  match lines:
    case Nil{}:
      acc
    case Con{+l, rest}:
      p_pls.go(rest,
        List.append(&2, PL, acc, Con{PL{l, p_kind_of(l)}, Nil{}}))

def p_pls(+s: String) -> List<&2, PL>:
  p_pls.go(String.split(s, p_nl()), Nil{})

# ---- shared folds ----

def p_join(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+x, +rest}:
      match rest:
        case Nil{}:
          x
        case Con{_, _}:
          String.append(x, String.append("\n", p_join(rest)))

# close the accumulated old/new line lists into one chunk
# (both lists are reversed: they were built by prepending)
def p_chunk_mk(+old: List<&2, String>, +new: List<&2, String>) -> PChunk:
  PChunk{List.reverse(&2, String, old), List.reverse(&2, String, new),
    List.length(&2, String, old), List.length(&2, String, new)}

# close an Add section: zero '+' lines give an empty file, otherwise
# every line carries a newline (Codex pushes '\n' per line)
def p_add_close(+path: String, +aacc: List<&2, String>) -> PAction:
  match aacc:
    case Nil{}:
      PAdd{path, ""}
    case Con{_, _}:
      PAdd{path,
        String.append(p_join(List.reverse(&2, String, aacc)), "\n")}

# the path after a section header - the starred form and the bare
# live-model form both parse (the drop length differs by 4)
def p_hdr_add.star(b: Bool, +l: String) -> String:
  match b:
    case True{}:
      String.drop(l, String.length("*** Add File: "))
    case False{}:
      String.drop(l, String.length("Add File: "))

def p_hdr_add(+l: String) -> String:
  p_hdr_add.star(String.starts_with(l, "*** Add File: "), l)

def p_hdr_del.star(b: Bool, +l: String) -> String:
  match b:
    case True{}:
      String.drop(l, String.length("*** Delete File: "))
    case False{}:
      String.drop(l, String.length("Delete File: "))

def p_hdr_del(+l: String) -> String:
  p_hdr_del.star(String.starts_with(l, "*** Delete File: "), l)

def p_hdr_upd.star(b: Bool, +l: String) -> String:
  match b:
    case True{}:
      String.drop(l, String.length("*** Update File: "))
    case False{}:
      String.drop(l, String.length("Update File: "))

def p_hdr_upd(+l: String) -> String:
  p_hdr_upd.star(String.starts_with(l, "*** Update File: "), l)

# ---- the actions loop ----
# One mode-parametrized pass over the annotated lines. Every recursive
# call shrinks `lines`. Sections close INLINE (a close-and-continue
# helper would need to call back into this loop - a forward reference,
# which this repo forbids - so the close matrix is spelled out per
# kind; the laws pin every branch).

type PMode is Data:
  MNorm{}
  MAdd{path: String, acc: List<&2, String>}
  MUpd{path: String, old: List<&2, String>, new: List<&2, String>,
    chunks: List<&2, PChunk>}
  MEnd{}

def p_actions.go(lines: List<&2, PL>, +acc: List<&2, PAction>,
    mode: PMode, bad: Bool) -> Maybe<&2, List<&2, PAction>>:
  match lines:
    case Nil{}:
      match mode:
        case MEnd{}:
          match bad:
            case True{}:
              None{}
            case False{}:
              Some{List.reverse(&2, PAction, acc)}
        case _:
          # the patch ended without '*** End Patch' - loud, like Codex
          None{}
    case Con{PL{+l, +kind}, rest}:
      match kind:
        case KBegin{}:
          # '*** Begin Patch' is only legal as the first line
          None{}
        case KGarbage{}:
          None{}
        case KAdd{}:
          match mode:
            case MAdd{+path, +aacc}:
              p_actions.go(rest, acc,
                MAdd{path, Con{String.drop(l, 1n), aacc}}, bad)
            case MUpd{+path, +old, +new, +chunks}:
              p_actions.go(rest, acc,
                MUpd{path, old, Con{String.drop(l, 1n), new}, chunks}, bad)
            case _:
              # a '+' line outside an Add or Update section
              None{}
        case KDel{}:
          match mode:
            case MUpd{+path, +old, +new, +chunks}:
              p_actions.go(rest, acc,
                MUpd{path, Con{String.drop(l, 1n), old}, new, chunks}, bad)
            case _:
              None{}
        case KCtx{}:
          match mode:
            case MUpd{+path, +old, +new, +chunks}:
              p_actions.go(rest, acc,
                MUpd{path, Con{String.drop(l, 1n), old},
                  Con{String.drop(l, 1n), new}, chunks}, bad)
            case _:
              None{}
        case KEmpty{}:
          match mode:
            case MUpd{+path, +old, +new, +chunks}:
              p_actions.go(rest, acc,
                MUpd{path, Con{"", old}, Con{"", new}, chunks}, bad)
            case MEnd{}:
              # trailing empty lines after End Patch are tolerated
              p_actions.go(rest, acc, MEnd{}, bad)
            case _:
              None{}
        case KHead{}:
          match mode:
            case MUpd{+path, +old, +new, +chunks}:
              match old:
                case Nil{}:
                  match new:
                    case Nil{}:
                      # no chunk is open: the @@ opens one (its hint
                      # text is dropped - the matcher never sees it)
                      p_actions.go(rest, acc,
                        MUpd{path, Nil{}, Nil{}, chunks}, bad)
                    case Con{_, _}:
                      # a chunk was open and non-empty: close it, open the next
                      p_actions.go(rest, acc,
                        MUpd{path, Nil{}, Nil{},
                          Con{p_chunk_mk(old, new), chunks}}, bad)
                case Con{_, _}:
                  p_actions.go(rest, acc,
                    MUpd{path, Nil{}, Nil{},
                      Con{p_chunk_mk(old, new), chunks}}, bad)
            case _:
              None{}
        case KEnd{}:
          match mode:
            case MNorm{}:
              p_actions.go(rest, acc, MEnd{}, bad)
            case MAdd{+path, +aacc}:
              p_actions.go(rest, Con{p_add_close(path, aacc), acc},
                MEnd{}, bad)
            case MUpd{+path, +old, +new, +chunks}:
              match old:
                case Nil{}:
                  match new:
                    case Nil{}:
                      match chunks:
                        # an Update File with no chunk is malformed
                        case Nil{}:
                          p_actions.go(rest, acc, MEnd{}, True{})
                        case Con{_, _}:
                          p_actions.go(rest,
                            Con{PUpdate{path,
                              List.reverse(&2, PChunk, chunks)}, acc},
                            MEnd{}, bad)
                    case Con{_, _}:
                      p_actions.go(rest,
                        Con{PUpdate{path,
                          List.reverse(&2, PChunk,
                            Con{p_chunk_mk(old, new), chunks})}, acc},
                        MEnd{}, bad)
                case Con{_, _}:
                  p_actions.go(rest,
                    Con{PUpdate{path,
                      List.reverse(&2, PChunk,
                        Con{p_chunk_mk(old, new), chunks})}, acc},
                    MEnd{}, bad)
            case MEnd{}:
              # a second '*** End Patch'
              None{}
        case KAddHdr{}:
          match mode:
            case MNorm{}:
              p_actions.go(rest, acc, MAdd{p_hdr_add(l), Nil{}}, bad)
            case MAdd{+path, +aacc}:
              p_actions.go(rest, Con{p_add_close(path, aacc), acc},
                MAdd{p_hdr_add(l), Nil{}}, bad)
            case MUpd{+path, +old, +new, +chunks}:
              match old:
                case Nil{}:
                  match new:
                    case Nil{}:
                      match chunks:
                        case Nil{}:
                          # empty Update File section: malformed
                          p_actions.go(rest, acc,
                            MAdd{p_hdr_add(l), Nil{}}, True{})
                        case Con{_, _}:
                          p_actions.go(rest,
                            Con{PUpdate{path,
                              List.reverse(&2, PChunk, chunks)}, acc},
                            MAdd{p_hdr_add(l), Nil{}}, bad)
                    case Con{_, _}:
                      p_actions.go(rest,
                        Con{PUpdate{path,
                          List.reverse(&2, PChunk,
                            Con{p_chunk_mk(old, new), chunks})}, acc},
                        MAdd{p_hdr_add(l), Nil{}}, bad)
                case Con{_, _}:
                  p_actions.go(rest,
                    Con{PUpdate{path,
                      List.reverse(&2, PChunk,
                        Con{p_chunk_mk(old, new), chunks})}, acc},
                    MAdd{p_hdr_add(l), Nil{}}, bad)
            case MEnd{}:
              None{}
        case KDelHdr{}:
          match mode:
            case MNorm{}:
              p_actions.go(rest, Con{PDelete{p_hdr_del(l)}, acc},
                MNorm{}, bad)
            case MAdd{+path, +aacc}:
              p_actions.go(rest,
                Con{PDelete{p_hdr_del(l)},
                  Con{p_add_close(path, aacc), acc}},
                MNorm{}, bad)
            case MUpd{+path, +old, +new, +chunks}:
              match old:
                case Nil{}:
                  match new:
                    case Nil{}:
                      match chunks:
                        case Nil{}:
                          p_actions.go(rest,
                            Con{PDelete{p_hdr_del(l)}, acc},
                            MNorm{}, True{})
                        case Con{_, _}:
                          p_actions.go(rest,
                            Con{PDelete{p_hdr_del(l)},
                              Con{PUpdate{path,
                                List.reverse(&2, PChunk, chunks)}, acc}},
                            MNorm{}, bad)
                    case Con{_, _}:
                      p_actions.go(rest,
                        Con{PDelete{p_hdr_del(l)},
                          Con{PUpdate{path,
                            List.reverse(&2, PChunk,
                              Con{p_chunk_mk(old, new), chunks})}, acc}},
                        MNorm{}, bad)
                case Con{_, _}:
                  p_actions.go(rest,
                    Con{PDelete{p_hdr_del(l)},
                      Con{PUpdate{path,
                        List.reverse(&2, PChunk,
                          Con{p_chunk_mk(old, new), chunks})}, acc}},
                    MNorm{}, bad)
            case MEnd{}:
              None{}
        case KUpdHdr{}:
          match mode:
            case MNorm{}:
              p_actions.go(rest, acc,
                MUpd{p_hdr_upd(l), Nil{}, Nil{}, Nil{}}, bad)
            case MAdd{+path, +aacc}:
              p_actions.go(rest, Con{p_add_close(path, aacc), acc},
                MUpd{p_hdr_upd(l), Nil{}, Nil{}, Nil{}}, bad)
            case MUpd{+path, +old, +new, +chunks}:
              match old:
                case Nil{}:
                  match new:
                    case Nil{}:
                      match chunks:
                        case Nil{}:
                          p_actions.go(rest, acc,
                            MUpd{p_hdr_upd(l), Nil{}, Nil{}, Nil{}},
                            True{})
                        case Con{_, _}:
                          p_actions.go(rest,
                            Con{PUpdate{path,
                              List.reverse(&2, PChunk, chunks)}, acc},
                            MUpd{p_hdr_upd(l), Nil{}, Nil{}, Nil{}},
                            bad)
                    case Con{_, _}:
                      p_actions.go(rest,
                        Con{PUpdate{path,
                          List.reverse(&2, PChunk,
                            Con{p_chunk_mk(old, new), chunks})}, acc},
                        MUpd{p_hdr_upd(l), Nil{}, Nil{}, Nil{}}, bad)
                case Con{_, _}:
                  p_actions.go(rest,
                    Con{PUpdate{path,
                      List.reverse(&2, PChunk,
                        Con{p_chunk_mk(old, new), chunks})}, acc},
                    MUpd{p_hdr_upd(l), Nil{}, Nil{}, Nil{}}, bad)
            case MEnd{}:
              None{}

def p_parse.go(lines: List<&2, PL>) -> Maybe<&2, List<&2, PAction>>:
  match lines:
    case Nil{}:
      None{}
    case Con{PL{+l, +kind}, rest}:
      match kind:
        case KBegin{}:
          p_actions.go(rest, Nil{}, MNorm{}, False{})
        case _:
          # the first line must be '*** Begin Patch'
          None{}

def parse_patch(+patch: String) -> Maybe<&2, List<&2, PAction>>:
  p_parse.go(p_pls(String.trim(patch)))

# the file path an action touches (the validate loop reads ahead: the
# next iteration's file is opened inside the previous do-block)
def p_path_of(act: PAction) -> String:
  match act:
    case PAdd{+path, _}:
      path
    case PDelete{+path}:
      path
    case PUpdate{+path, _}:
      path

# ---- the UPDATE matcher (strict, sequential, first-occurrence) ----

type PFnd is Data:
  PFnd{rel: Nat, lines: List<&2, String>}

type POut is Data:
  POut{ok: Bool, content: String, failed: Nat}

# eq of the two heads (routed: the equality is computed, so it rides
# as a Bool argument)
def p_head_eq(+lines: List<&2, String>, +old: List<&2, String>) -> Bool:
  match lines:
    case Nil{}:
      False{}
    case Con{+x, _}:
      match old:
        case Nil{}:
          True{}
        case Con{+o, _}:
          String.eq(x, o)

# does `lines` start with the line sequence `old`? (b: are the two
# heads equal - computed by the caller, never a computed scrutinee)
def p_seq_head(lines: List<&2, String>, old: List<&2, String>,
    b: Bool) -> Bool:
  match lines:
    case Nil{}:
      match old:
        case Nil{}:
          True{}
        case Con{_, _}:
          False{}
    case Con{_, +xrest}:
      match old:
        case Nil{}:
          True{}
        case Con{_, +orst}:
          match b:
            case True{}:
              p_seq_head(xrest, orst, p_head_eq(xrest, orst))
            case False{}:
              False{}

# the same question for a whole search position (the entry that also
# handles the empty-sequence case)
def p_seqf(+lines: List<&2, String>, +old: List<&2, String>) -> Bool:
  match old:
    case Nil{}:
      True{}
    case Con{_, _}:
      p_seq_head(lines, old, p_head_eq(lines, old))

def p_splice(+lines: List<&2, String>, +pos: Nat, +olen: Nat,
    +new: List<&2, String>) -> List<&2, String>:
  List.append(&2, String, List.take(&2, String, lines, pos),
    List.append(&2, String, new,
      List.drop(&2, String, lines, Nat.add(pos, olen))))

def p_replace.bump(+h: String, m: Maybe<&2, PFnd>) -> Maybe<&2, PFnd>:
  match m:
    case Some{+f}:
      match f:
        case PFnd{+rel, +ls}:
          Some{PFnd{Nat.add(rel, 1n), Con{h, ls}}}
    case None{}:
      None{}

# replace the first occurrence of `old` in `lines` with `new`;
# None when `old` occurs nowhere (the miss that must fail loudly)
def p_replace.go(+lines: List<&2, String>, +old: List<&2, String>,
    +new: List<&2, String>, +olen: Nat, b: Bool) -> Maybe<&2, PFnd>:
  match lines:
    case Nil{}:
      match b:
        case True{}:
          Some{PFnd{0n, p_splice(Nil{}, 0n, olen, new)}}
        case False{}:
          None{}
    case Con{+h, +rest}:
      match b:
        case True{}:
          Some{PFnd{0n, p_splice(lines, 0n, olen, new)}}
        case False{}:
          p_replace.bump(h,
            p_replace.go(rest, old, new, olen, p_seqf(rest, old)))

def p_replace(+lines: List<&2, String>, +old: List<&2, String>,
    +new: List<&2, String>, +olen: Nat) -> Maybe<&2, PFnd>:
  p_replace.go(lines, old, new, olen, p_seqf(lines, old))

# the last chunk closes the loop: tail2 already carries its splice
def p_apply.last(+done: List<&2, String>, +index: Nat,
    m: Maybe<&2, PFnd>) -> POut:
  match m:
    case Some{+f}:
      match f:
        case PFnd{_, +tail2}:
          POut{True{},
            p_join(List.append(&2, String, done, tail2)),
            Nat.add(index, 1n)}
    case None{}:
      POut{False{}, "", index}

# the chunk loop: `done` holds the already-spliced lines, `todo` the
# region the NEXT chunk may match in (strict sequential), `m` is the
# search result for the head chunk, computed by the previous
# iteration (or the boot) as an argument
def p_apply.go(chunks: List<&2, PChunk>, +done: List<&2, String>,
    +todo: List<&2, String>, +index: Nat, m: Maybe<&2, PFnd>) -> POut:
  match chunks:
    case Nil{}:
      POut{True{}, p_join(done), index}
    case Con{+chunk, +crest}:
      match chunk:
        case PChunk{_, +new, _, +nlen}:
          match crest:
            case Nil{}:
              p_apply.last(done, index, m)
            case Con{+nchunk, _}:
              match nchunk:
                case PChunk{+nold, +nnew, +nolen, _}:
                  match m:
                    case Some{+f}:
                      match f:
                        case PFnd{+rel, +tail2}:
                          p_apply.go(crest,
                            List.append(&2, String, done,
                              List.append(&2, String,
                                List.take(&2, String, tail2, rel), new)),
                            List.drop(&2, String, tail2,
                              Nat.add(rel, nlen)),
                            Nat.add(index, 1n),
                            p_replace(
                              List.drop(&2, String, tail2,
                                Nat.add(rel, nlen)),
                              nold, nnew, nolen))
                    case None{}:
                      # the chunk's lines are NOT in the remaining file region
                      POut{False{}, "", index}

def p_apply_boot(+lines: List<&2, String>, +chunks: List<&2, PChunk>)
    -> POut:
  match chunks:
    case Nil{}:
      POut{True{}, p_join(lines), 0n}
    case Con{+chunk, _}:
      match chunk:
        case PChunk{+old, +new, +olen, +nlen}:
          p_apply.go(chunks, Nil{}, lines, 0n,
            p_replace(lines, old, new, olen))

def apply_update(+content: String, +chunks: List<&2, PChunk>) -> POut:
  p_apply_boot(String.split(content, p_nl()), chunks)

# the first context/removed line of the failed chunk (the error
# message's anchor, so the model knows what to re-read)
def p_chunk_anchor.at(tail: List<&2, PChunk>) -> String:
  match tail:
    case Nil{}:
      ""
    case Con{PChunk{+old, _, _, _}, _}:
      match old:
        case Nil{}:
          ""
        case Con{+a, _}:
          a

def p_chunk_anchor(+chunks: List<&2, PChunk>, +index: Nat) -> String:
  p_chunk_anchor.at(List.drop(&2, PChunk, chunks, index))
