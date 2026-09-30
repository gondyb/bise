# runtime/settings-pure.bend - the pure half of runtime/settings.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base

def CONFIG_TEMPLATE() -> String:
  "# bend-harness configuration.\n# Precedence on every key: environment variable > this file > built-in default.\n\n# The model to use, \"provider/model\" (bise models lists them). No model:\n# bise asks for a provider and checks its key at the next launch.\n# model = \"anthropic/claude-opus-5-5\"\n\n# The compaction threshold, in estimated tokens (default: 80% of the\n# model's context window, from the model catalog)\n# threshold = 800000\n\n# A bash command still running after this many seconds moves to the\n# background and the agent keeps going (default 30)\nbg_after = 30\n"
