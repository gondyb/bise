# runtime/model.bend — the Runtime's provider adapter.
#
# The Core never performs provider I/O; it emits AgentCall and CompactionCall
# Actions and the Runtime answers with one completion command each. This
# adapter is scripted: it pops replies in order, which makes every scenario
# deterministic. A real provider adapter (HTTP, streaming, cancellation,
# retries) would replace this file without touching the Core.
import Base
import ../core/types.bend as T

type Reply is Data:
  ROk{text: String, calls: List<&2, T.Call>}
  RFail{}

# one popped reply plus the remaining script
type Stepped is Data:
  Stepped{reply: Reply, rest: List<&2, Reply>}

# pops the next scripted reply; an exhausted script fails like a provider
def model_next(script: List<&2, Reply>) -> Stepped:
  match script:
    case Con{r, rest}:
      Stepped{r, rest}
    case Nil{}:
      Stepped{RFail{}, Nil{}}
