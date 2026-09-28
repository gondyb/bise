# PROOF.bend — proofs of the harness laws. Each holds by unfolding the
# definitions involved; the checker verifies the equalities.
import Base
import ./core/types.bend as T
import ./core/history.bend as H
import ./core/compaction.bend as C
import ./core/checkpoint.bend as K
import ./core/session.bend as S
import ./core/program.bend as P
import ./vendor/json.bend as J
import ./hub/model.bend as HM
import ./hub/cx.bend as HX
import ./hub/core.bend as HC
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

def Laws.compaction_request_ends_with_prompt():
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

def Laws.wire_args_reversible():
  {==}

def Laws.wire_args_keep_backslash_n():
  {==}

def Laws.wire_args_keep_sentinel():
  {==}

def Laws.reply_encodes_multiline_args():
  {==}

def Laws.parse_reply_decodes_multiline_args():
  {==}

def Laws.wire_echo_encodes_multiline_args():
  {==}

def Laws.api_wire_decodes_args():
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

def Laws.compaction_preview_one_line():
  {==}

def Laws.compaction_preview_unescapes():
  {==}

def Laws.implicit_say():
  {==}

def Laws.raw_words_pass_through():
  {==}

def Laws.slash_compact_maps():
  {==}

def Laws.slash_steer_is_not_a_command():
  {==}

def Laws.slash_notify_is_not_a_command():
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

def Laws.glm_thinking_surfaces():
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

def Laws.checkpoint_text_roundtrip_queued():
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

def Laws.bg_wrapper_script():
  {==}

def Laws.bg_after_defaults():
  {==}

def Laws.bg_after_clamps_high():
  {==}

def Laws.bg_after_keeps_low():
  {==}

def Laws.exec_of_background_maps_ok():
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

def Laws.retryable_status_transient():
  {==}

def Laws.retryable_err_midstream():
  {==}

def Laws.provider_max_attempts():
  {==}

def Laws.provider_backoff_schedule():
  {==}

def Laws.provider_retry_after():
  {==}

def Laws.provider_529_retries_with_hint():
  {==}

def Laws.provider_200_error_body_retries():
  {==}

def Laws.provider_ok_passes_through():
  {==}

def Laws.provider_client_error_is_final():
  {==}

def Laws.provider_retry_line():
  {==}

def Laws.provider_exhausted_text():
  {==}

def Laws.exec_of_exit_still_fails():
  {==}

def Laws.slash_reload_maps():
  {==}

def Laws.reload_raw_word():
  {==}

def Laws.reload_allowed_when_fresh(cfg):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.reload_refused_while_turn(cfg, text):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      match text:
        case "":
          {==}
        case _:
          {==}

def Laws.reload_allows_with_queue(cfg, text):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      match text:
        case "":
          {==}
        case _:
          {==}

def Laws.reload_allows_with_notif(cfg, text):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      match text:
        case "":
          {==}
        case _:
          {==}

def Laws.bg_dir_default_root():
  {==}

def Laws.bg_dir_custom_root():
  {==}

def Laws.bg_slot_paths():
  {==}

def Laws.bg_poll_iters_table():
  {==}

def Laws.bg_headline_names_window_id_dir():
  {==}

def Laws.bg_tail_cmd_concrete():
  {==}

def Laws.bg_stdin_cmd_concrete():
  {==}

def Laws.bg_status_cmd_concrete():
  {==}

def Laws.bg_kill_cmd_concrete():
  {==}

def Laws.bg_wrapper_fifo_rdrw():
  {==}

def Laws.bg_wrapper_runner_waits_and_records():
  {==}

def Laws.bg_wrapper_output_is_a_file():
  {==}

def Laws.bg_wrapper_never_kills():
  {==}

def Laws.bg_wrapper_cleans_slot():
  {==}

def Laws.bg_wrapper_handoff_marker():
  {==}

def Laws.exec_of_success_is_ok():
  {==}

def Laws.exec_of_failure_carries_code():
  {==}

def Laws.exec_of_garbage_fails_closed():
  {==}

def Laws.exec_of_no_newline_fails_closed():
  {==}

def Laws.bg_setup_guard_names_the_dir():
  {==}

def Laws.bg_rc_guard_names_the_dir():
  {==}

def Laws.bg_wrapper_has_both_guards():
  {==}

def Laws.bg_script_hash_deterministic():
  {==}

def Laws.bg_script_hash_distinguishes():
  {==}

def Laws.bg_hash_fold_keeps_small():
  {==}

def Laws.bg_hash_pos_cycles():
  {==}

def Laws.bash_script_path_per_command():
  {==}
def Laws.self_reload_acks():
  {==}

def Laws.self_compact_acks():
  {==}

def Laws.self_tool_routing_table():
  {==}

def Laws.sleep_program_bounds():
  {==}

def Laws.sleep_program_secs():
  {==}

def Laws.self_acks_carry_markers():
  {==}

def Laws.deferred_reads_reload_request():
  {==}

def Laws.deferred_reads_compact_request():
  {==}

def Laws.deferred_reads_both_requests():
  {==}

def Laws.deferred_resets_at_user():
  {==}

def Laws.deferred_empty_history():
  {==}

def Laws.live_catalog_has_seven_tools():
  {==}

def Laws.parity_catalog_keeps_three():
  {==}

def Laws.live_catalog_skill_patch_and_self_names():
  {==}

def Laws.self_reload_desc_says_when():
  {==}

def Laws.self_compact_desc_says_when():
  {==}

def Laws.def_acks_suffix_empty_when_none():
  {==}

def Laws.def_acks_suffix_carries_reload():
  {==}

def Laws.def_acks_suffix_carries_both():
  {==}

def Laws.bg_wrapper_runner_frees_slot():
  {==}

def Laws.bg_wrapper_pid_wait_breaks_on_rc():
  {==}
def Laws.steering_lands_in_history():
  {==}

def Laws.steering_survives_interrupt():
  {==}

def Laws.steering_survives_fail():
  {==}

def Laws.steering_arrival_order():
  {==}

def Laws.exec_of_real_199_fails():
  {==}

def Laws.bg_wrapper_atomic_slot_allocation():
  {==}

def Laws.bg_wrapper_output_capped():
  {==}

def Laws.bg_wrapper_runner_ownership_check():
  {==}

def Laws.bg_wrapper_cleans_its_script():
  {==}

def Laws.bg_wrapper_multiline_command_intact():
  {==}

def Laws.bg_sentinel_derives_from_command():
  {==}
def Laws.mcp_alias_normalizes():
  {==}

def Laws.mcp_alias_dedupes():
  {==}

def Laws.mcp_bootstrap_url_parity():
  {==}

def Laws.steer_path_is_per_port():
  {==}

def Laws.steer_channel_skips_blank_lines():
  {==}

def Laws.drain_emits_steering_received():
  {==}

def Laws.drain_steering_is_act_free():
  {==}

def Laws.drain_next_of_rejected_keeps_session(s):
  {==}

def Laws.skills_rules_contract():
  {==}

def Laws.skills_escape_xml():
  {==}

def Laws.skills_entry_format():
  {==}

def Laws.skills_catalog_dedupes():
  {==}

def Laws.skills_catalog_empty_is_empty():
  {==}

def Laws.skills_malformed_lines_skipped():
  {==}

def Laws.skills_args_name_json():
  {==}

def Laws.skills_args_name_rejects_empty():
  {==}

def Laws.skills_miss_lists_available():
  {==}

def Laws.skills_find_exact():
  {==}

def Laws.skills_prompt_gated():
  {==}

def Laws.skills_strip_frontmatter():
  {==}

def Laws.merged_surface_carries_live_catalog():
  {==}

def Laws.queue_hold_is_act_free():
  {==}

def Laws.queue_holds_mid_turn():
  {==}

def Laws.queue_survives_turn_end():
  {==}

def Laws.queue_pops_at_idle():
  {==}

def Laws.queue_pop_removes_head():
  {==}

def Laws.queue_delivered_in_history():
  {==}

def Laws.api_body_high_reasoning():
  {==}

def Laws.asst_think_roundtrip():
  {==}

def Laws.asst_think_only_no_text_block():
  {==}

def Laws.asst_plain_content_stays_plain():
  {==}

def Laws.api_body_returns_thinking_block():
  {==}

def Laws.chunk_lines_split():
  {==}

def Laws.chunk_lines_empty():
  {==}

def Laws.interrupt_path_is_per_port():
  {==}

def Laws.say_multiline_unescapes():
  {==}

def Laws.ser_msg_escapes_newlines():
  {==}

def Laws.checkpoint_multiline_roundtrip():
  {==}

def Laws.apply_patch_desc_names_the_format():
  {==}

def Laws.patch_parse_add():
  {==}

def Laws.patch_parse_add_empty():
  {==}

def Laws.patch_parse_delete():
  {==}

def Laws.patch_parse_update():
  {==}

def Laws.patch_parse_implicit_chunk():
  {==}

def Laws.patch_parse_multi():
  {==}

def Laws.patch_rejects_missing_end():
  {==}

def Laws.patch_rejects_bad_first():
  {==}

def Laws.patch_rejects_empty_update():
  {==}

def Laws.patch_rejects_garbage_section():
  {==}

def Laws.patch_rejects_after_end():
  {==}

def Laws.patch_tolerates_trailing_newline():
  {==}

def Laws.patch_kind_add_line():
  {==}

def Laws.patch_kind_del_line():
  {==}

def Laws.patch_kind_ctx_line():
  {==}

def Laws.patch_kind_empty_line():
  {==}

def Laws.patch_kind_begin():
  {==}

def Laws.patch_kind_end():
  {==}

def Laws.patch_kind_add_header():
  {==}

def Laws.patch_kind_delete_header():
  {==}

def Laws.patch_kind_update_header():
  {==}

def Laws.patch_kind_chunk_head():
  {==}

def Laws.patch_kind_garbage():
  {==}

def Laws.patch_apply_hit():
  {==}

def Laws.patch_apply_miss():
  {==}

def Laws.patch_apply_sequential():
  {==}

def Laws.patch_apply_sequential_miss():
  {==}

def Laws.patch_apply_insert():
  {==}

def Laws.patch_anchor_names_the_chunk():
  {==}

def Laws.patch_path_of_actions():
  {==}

def Laws.patch_kind_begin_prefix():
  {==}

def Laws.patch_kind_bare_add_header():
  {==}

def Laws.patch_kind_bare_delete_header():
  {==}

def Laws.patch_kind_bare_update_header():
  {==}

def Laws.patch_parse_glm_variant():
  {==}

def Laws.interrupt_closes_waiting_calls():
  {==}

def Laws.interrupt_closes_prog_pending():
  {==}

def Laws.interrupt_closes_agent_pending():
  {==}

def Laws.deferred_ignores_assistant_prose():
  {==}

def Laws.deferred_ignores_source_quotes():
  {==}

def Laws.deferred_ignores_midtext_marker():
  {==}

def Laws.deferred_reads_prefixed_ack():
  {==}

def Laws.deferred_reads_program_acked_result():
  {==}

def Laws.provider_resolves_alias():
  {==}

def Laws.provider_opus_is_anthropic():
  {==}

def Laws.provider_glm_is_openai():
  {==}

def Laws.provider_anthropic_base():
  {==}

def Laws.provider_openai_base():
  {==}

def Laws.provider_anthropic_key_env():
  {==}

def Laws.provider_openai_key_env():
  {==}

def Laws.style_routes_body():
  {==}

def Laws.style_routes_reply():
  {==}

def Laws.api_body_anth_maps():
  {==}

def Laws.api_body_anth_bare_omits_tools():
  {==}

def Laws.api_body_anth_tools_shape():
  {==}

def Laws.anth_tool_result_pairs():
  {==}

def Laws.anth_tool_results_merge():
  {==}

def Laws.oai_parallel_results_pair_in_order():
  {==}

def Laws.anth_parallel_results_pair_in_order():
  {==}

def Laws.anth_unsigned_thinking_stays_stripped():
  {==}

def Laws.anth_signed_thinking_replays():
  {==}

def Laws.anth_multi_span_replays():
  {==}

def Laws.anth_refusal_maps():
  {==}

def Laws.anth_long_tool_run_keeps_messages():
  {==}

def Laws.glm_thinking_signature_surfaces():
  {==}

def Laws.glm_signed_think_signature_replays():
  {==}

def Laws.anth_blocks_join_lines():
  {==}

def Laws.anth_thinking_surfaces():
  {==}

def Laws.anth_tool_calls_map():
  {==}

def Laws.anth_structured_call_maps():
  {==}

def Laws.anth_error_body_maps():
  {==}

def Laws.anth_status_error_maps():
  {==}

def Laws.anth_tool_name_roundtrip():
  {==}

def Laws.config_parse_flat():
  {==}

def Laws.config_skips_comments_blanks():
  {==}

def Laws.config_sections_prefix():
  {==}

def Laws.config_value_keeps_equals():
  {==}

def Laws.config_lookup_found():
  {==}

def Laws.config_lookup_missing():
  {==}

def Laws.config_u32_parses():
  {==}

def Laws.config_u32_defaults():
  {==}

def Laws.config_env_wins():
  {==}

def Laws.config_env_empty_falls_to_file():
  {==}

def Laws.config_env_empty_no_file():
  {==}

def Laws.config_template_parses():
  {==}

def Laws.config_template_threshold():
  {==}

def Laws.info_line_format():
  {==}

def Laws.info_paths_are_the_drained_ones():
  {==}

def Laws.resume_replays_history():
  {==}
def Laws.workdir_empty_keeps_command():
  {==}
def Laws.workdir_cds_first():
  {==}
def Laws.context_empty_keeps_request():
  {==}
def Laws.context_is_last_user_message():
  {==}
def Laws.context_skips_compaction():
  {==}
def Laws.extra_prompt_appends():
  {==}
def Laws.extra_prompt_empty_keeps():
  {==}
def Laws.steer_line_unescapes():
  {==}

# ---- Switchboard hub ----

# a step context's state is the replay of its journal: the proof it carries
def hub_core_ok(-o: HM.St, c: HX.Core<o>) -> {HX.Core.d(o, c) == HM.apply_all(HX.journal(HX.Core.fx(o, c)), o) : HM.St}:
  match c:
    case HX.Core{d, fx, p, f, q}:
      p

def Laws.hub_step_replays(h, i):
  hub_core_ok(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))


def hub_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HM.St)
    -> {HM.apply_all(b, HM.apply_all(a, s)) == HM.apply_all(List.append(&2, HM.Ev, a, b), s) : HM.St}:
  match a:
    case Nil{}:
      {==}
    case Con{e, t}:
      hub_append(t, b, HM.apply(s, e))

def hub_commit_go(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> {HC.Hub.d(HC.Out.hub(HC.commit.go(HC.Hub.d(h), need, c, r, h)))
        == HM.apply_all(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)) : HM.St}:
  match need:
    case Some{q}:
      {==}
    case None{}:
      hub_core_ok(HC.Hub.d(h), c)

def hub_commit_cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> {HC.Hub.d(HC.Out.hub(HC.commit.cx(HC.Hub.d(h), cx, h)))
        == HM.apply_all(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)) : HM.St}:
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go(h, HX.R.need(r), c, r)

# what sb-core keeps of an input is the replay of its journal
def hub_commit_ok(+h: HC.Hub, +i: J.Json)
    -> {HC.Hub.d(HC.Out.hub(HC.commit(h, i))) == HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)) : HM.St}:
  hub_commit_cx(h, HC.step(h, i))

def Laws.hub_run_replays(ins, h):
  match ins:
    case Nil{}:
      {==}
    case Con{i, rest}:
      Equal.trans(HM.St,
        HC.Hub.d(HC.RunR.hub(HC.run(rest, HC.Out.hub(HC.commit(h, i))))),
        HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(HC.Out.hub(HC.commit(h, i)))),
        HM.apply_all(List.append(&2, HM.Ev, HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i))))), HC.Hub.d(h)),
        Laws.hub_run_replays(rest, HC.Out.hub(HC.commit(h, i))),
        Equal.trans(HM.St,
          HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(HC.Out.hub(HC.commit(h, i)))),
          HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h))),
          HM.apply_all(List.append(&2, HM.Ev, HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i))))), HC.Hub.d(h)),
          Equal.cong(HM.St, HM.St, x => HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), x),
            HC.Hub.d(HC.Out.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)),
            hub_commit_ok(h, i)),
          hub_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(h))))

def hub_fresh_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HM.St, fa: HX.Fresh(a, s),
    fb: HX.Fresh(b, HM.apply_all(a, s))) -> HX.Fresh(List.append(&2, HM.Ev, a, b), s):
  match a:
    case Nil{}:
      fb
    case Con{e, t}:
      match fa:
        case HX.Both{x, y}:
          HX.Both{x, hub_fresh_append(t, b, HM.apply(s, e), y, fb)}

def hub_quiet_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, qa: HX.Quiet(a), qb: HX.Quiet(b))
    -> HX.Quiet(List.append(&2, HM.Ev, a, b)):
  match a:
    case Nil{}:
      qb
    case Con{e, t}:
      match qa:
        case HX.Both{x, y}:
          HX.Both{x, hub_quiet_append(t, b, y, qb)}

def hub_fresh_tr(-evs: List<&2, HM.Ev>, -x: HM.St, -y: HM.St, e: {x == y : HM.St}, f: HX.Fresh(evs, x)) -> HX.Fresh(evs, y):
  %e : HX.Fresh(evs, _)
  f

def hub_commit_go_f(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> HX.Both<HX.Fresh(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)),
               HX.Quiet(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))))>:
  match need:
    case Some{q}:
      HX.Both{Unit{}, Unit{}}
    case None{}:
      match c:
        case HX.Core{d, fx, p, f, q}:
          HX.Both{f, q}

def hub_commit_f.cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> HX.Both<HX.Fresh(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)),
               HX.Quiet(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))))>:
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go_f(h, HX.R.need(r), c, r)

def hub_commit_f(+h: HC.Hub, +i: J.Json)
    -> HX.Both<HX.Fresh(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Out.fx(HC.commit(h, i))))>:
  hub_commit_f.cx(h, HC.step(h, i))

def hub_fst(-A: Data, -B: Data, x: HX.Both<A, B>) -> A:
  match x:
    case HX.Both{a, b}:
      a

def hub_snd(-A: Data, -B: Data, x: HX.Both<A, B>) -> B:
  match x:
    case HX.Both{a, b}:
      b

def Laws.msg_ids_fresh(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      hub_fresh_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(h),
        hub_fst(HX.Fresh(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Out.fx(HC.commit(h, i)))), hub_commit_f(h, i)),
        hub_fresh_tr(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))),
          HC.Hub.d(HC.Out.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)),
          hub_commit_ok(h, i), Laws.msg_ids_fresh(rest, HC.Out.hub(HC.commit(h, i)))))

def Laws.auto_never_asks(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      hub_quiet_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))),
        hub_snd(HX.Fresh(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Out.fx(HC.commit(h, i)))), hub_commit_f(h, i)),
        Laws.auto_never_asks(rest, HC.Out.hub(HC.commit(h, i))))

def hub_set_waiters(-o: HM.St, cx: HX.Cx<o>, +ws: List<&2, HX.Waiter>)
    -> {HC.waiters_of_cx(o, HC.set_waiters(o, cx, ws)) == ws : List<&2, HX.Waiter>}:
  match cx:
    case HX.Cx{c, r}:
      match r:
        case HX.R{a, b, st, d, e, f, g, h, i, j, k, l}:
          {==}

def hub_later_tr(-x: List<&2, HX.Waiter>, -y: List<&2, HX.Waiter>, -now: Nat, e: {x == y : List<&2, HX.Waiter>},
    l: HC.Later(y, now)) -> HC.Later(x, now):
  %Equal.sym(List<&2, HX.Waiter>, x, y, e) : HC.Later(_, now)
  l

def hub_keep_later.pick(c: Bool, +w: HX.Waiter, -t: List<&2, HX.Waiter>, -now: Nat,
    +e: {Nat.is_le(HX.Waiter.deadline(w), now) == c : Bool}, ih: HC.Later(HC.waiters_keep(t, now), now))
    -> HC.Later(Bool.pick(List<&2, HX.Waiter>, Nat.is_le(HX.Waiter.deadline(w), now), HC.waiters_keep(t, now), w <> HC.waiters_keep(t, now)), now):
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_le(HX.Waiter.deadline(w), now), True{}, e) : HC.Later(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_keep(t, now), w <> HC.waiters_keep(t, now)), now)
      ih
    case False{}:
      %Equal.sym(Bool, Nat.is_le(HX.Waiter.deadline(w), now), False{}, e) : HC.Later(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_keep(t, now), w <> HC.waiters_keep(t, now)), now)
      HX.Both{e, ih}

def hub_keep_later(+ws: List<&2, HX.Waiter>, +now: Nat) -> HC.Later(HC.waiters_keep(ws, now), now):
  match ws:
    case Nil{}:
      Unit{}
    case Con{w, t}:
      hub_keep_later.pick(Nat.is_le(HX.Waiter.deadline(w), now), w, t, now, {==}, hub_keep_later(t, now))

def Laws.tick_clears_late_waiters(o, cx):
  hub_later_tr(HC.waiters_of_cx(o, HC.tick(o, cx)), HC.tick.keep(o, cx), HX.R.now(HX.Cx.r(o, cx)),
    hub_set_waiters(o, HC.tick.go(o, cx), HC.tick.keep(o, cx)),
    hub_keep_later(HX.R.waiters(HX.Cx.r(o, cx)), HX.R.now(HX.Cx.r(o, cx))))

def hub_none_tr(-x: List<&2, HX.Waiter>, -y: List<&2, HX.Waiter>, -n: String, e: {x == y : List<&2, HX.Waiter>},
    l: HC.NoneOf(y, n)) -> HC.NoneOf(x, n):
  %Equal.sym(List<&2, HX.Waiter>, x, y, e) : HC.NoneOf(_, n)
  l

def hub_not_none.pick(c: Bool, +w: HX.Waiter, -t: List<&2, HX.Waiter>, -n: String,
    +e: {String.eq(HX.Waiter.agent(w), n) == c : Bool}, ih: HC.NoneOf(HC.waiters_not(t, n), n))
    -> HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, String.eq(HX.Waiter.agent(w), n), HC.waiters_not(t, n), w <> HC.waiters_not(t, n)), n):
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HX.Waiter.agent(w), n), True{}, e) : HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_not(t, n), w <> HC.waiters_not(t, n)), n)
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(HX.Waiter.agent(w), n), False{}, e) : HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_not(t, n), w <> HC.waiters_not(t, n)), n)
      HX.Both{e, ih}

def hub_not_none(+ws: List<&2, HX.Waiter>, +n: String) -> HC.NoneOf(HC.waiters_not(ws, n), n):
  match ws:
    case Nil{}:
      Unit{}
    case Con{w, t}:
      hub_not_none.pick(String.eq(HX.Waiter.agent(w), n), w, t, n, {==}, hub_not_none(t, n))

def Laws.stop_leaves_no_waiter(o, cx, n, life, why):
  hub_none_tr(HC.waiters_of_cx(o, HC.stop_task(o, cx, n, life, why)), HC.waiters_not(HX.R.waiters(HX.Cx.r(o, cx)), n), n,
    hub_set_waiters(o, HC.stop_task.go(o, cx, n, life, why), HC.waiters_not(HX.R.waiters(HX.Cx.r(o, cx)), n)),
    hub_not_none(HX.R.waiters(HX.Cx.r(o, cx)), n))
