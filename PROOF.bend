# PROOF.bend — proofs of the harness laws. Each holds by unfolding the
# definitions involved; the checker verifies the equalities.
import Base
import ./core/types.bend as T
import ./core/history.bend as H
import ./core/compaction.bend as C
import ./core/checkpoint.bend as K
import ./core/session.bend as S
import ./core/program.bend as P
import ./LAWS.bend as Laws

def Laws.checkpoint_roundtrip(s):
  {==}

def Laws.summary_rejects_tool_calls(t, c):
  {==}

def Laws.empty_block_malformed():
  {==}

def Laws.fallback_accepts_plain_text():
  {==}

def Laws.drop_one_empty():
  {==}

def Laws.manual_instructions_tagged():
  {==}

def Laws.model_input_leads_with_system(cfg, h):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.run_consumes_in_order(n1, a1, n2, a2, r1, r2):
  {==}

def Laws.run_waits_at_the_missing_result(n1, a1, n2, a2, r1):
  {==}

def Laws.parse_roundtrip():
  {==}

def Laws.parse_ignores_types():
  {==}

def Laws.parse_rejects_garbage():
  {==}

def Laws.branch_skips_untaken_calls():
  {==}

def Laws.commits_append_at_tail():
  {==}

def Laws.wire_call_roundtrip():
  {==}

def Laws.wire_reply_roundtrip():
  {==}

def Laws.orphan_tool_result_rejected(cfg):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.idle_interrupt_noop(cfg):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.truncate_marks_the_cut():
  {==}

def Laws.truncate_keeps_small_messages():
  {==}

def Laws.preserve_recent_keeps_users():
  {==}

def Laws.obs_wire_format():
  {==}

def Laws.implicit_say():
  {==}

def Laws.raw_words_pass_through():
  {==}

def Laws.slash_compact_maps():
  {==}

def Laws.slash_steer_maps():
  {==}

def Laws.slash_steer_needs_arg():
  {==}

def Laws.slash_quit_maps():
  {==}

def Laws.unknown_slash_refused():
  {==}

def Laws.api_wire_parses():
  {==}

def Laws.api_body_maps():
  {==}

def Laws.glm_blocks_join_lines():
  {==}

def Laws.api_tool_calls_map():
  {==}

def Laws.api_error_maps():
  {==}

def Laws.unclosed_summary_falls_back():
  {==}

def Laws.escape_keeps_lines_single():
  {==}

def Laws.escape_roundtrip():
  {==}

def Laws.api_bare_body_omits_tools():
  {==}

def Laws.checkpoint_text_roundtrip():
  {==}

def Laws.renumber_is_sequential():
  {==}

def Laws.renumber_keeps_length(c1, c2, base):
  match c1:
    case T.Call{i1, n1, a1}:
      match c2:
        case T.Call{i2, n2, a2}:
          {==}

def Laws.wire_echo_renames_node_program():
  {==}

def Laws.wire_echo_keeps_names():
  {==}

def Laws.result_text_renames_node_program():
  {==}

def Laws.result_text_keeps_names():
  {==}

def Laws.args_json_passes_structured():
  {==}

def Laws.args_json_passes_search():
  {==}

def Laws.args_json_wraps_plain():
  {==}

def Laws.call_code_extracts():
  {==}

def Laws.call_code_passthrough():
  {==}

def Laws.best_match_names_only():
  {==}

def Laws.best_match_ranks_words():
  {==}

def Laws.best_match_caps_and_flags():
  {==}

def Laws.details_missing_is_loud():
  {==}

def Laws.details_show_found_and_missing():
  {==}

def Laws.details_alias_resolves():
  {==}

def Laws.flat_details_still_works():
  {==}

def Laws.best_match_connectors_filter():
  {==}

def Laws.tool_ann_format():
  {==}

def Laws.tool_ann_renames_node_program():
  {==}

def Laws.tool_res_ann_ok():
  {==}

def Laws.tool_res_ann_fail():
  {==}

def Laws.subtool_ann_format():
  {==}

def Laws.ann_flat_single_line():
  {==}

def Laws.ann_flat_caps_at_200():
  {==}

def Laws.node_err_extracts_message():
  {==}

def Laws.bash_watchdog_script():
  {==}

def Laws.bash_timeout_defaults():
  {==}

def Laws.bash_timeout_clamps_high():
  {==}

def Laws.bash_timeout_keeps_low():
  {==}

def Laws.bash_timeout_exit_is_readable():
  {==}

def Laws.cap_out_keeps_small():
  {==}

def Laws.cap_out_limit_is_20000():
  {==}

def Laws.cap_out_at_keeps_small():
  {==}

def Laws.cap_out_at_truncates():
  {==}

def Laws.retryable_status_table():
  {==}

def Laws.retryable_err_table():
  {==}

def Laws.attempt_timeout_retries():
  {==}

def Laws.attempt_tls_fails():
  {==}
