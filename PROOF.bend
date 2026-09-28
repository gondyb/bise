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

import ./hub/inv.bend as HI

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
    case HX.Core{d, fx, p, f, q, v}:
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
        case HX.Core{d, fx, p, f, q, v}:
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

# ---- L2: each message is one record (msgs_unique) ----

def hub_bdisc(b: Bool) -> Type:
  match b:
    case True{}:
      Unit
    case False{}:
      Empty

def hub_ft_absurd(e: {False{} == True{} : Bool}) -> Empty:
  %Equal.sym(Bool, False{}, True{}, e) : hub_bdisc(_)
  Unit{}

def hub_absurd_eq(x: Empty, -A: Type, -a: A, -b: A) -> {a == b : A}:
  match x:

# y < x: x is not y
def hub_lt_neq(+y: Nat, +x: Nat, h: {Nat.is_lt(y, x) == True{} : Bool}) -> {Nat.is_eq(x, y) == False{} : Bool}:
  match y x:
    case 0n 0n:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_eq(x, y), False{})
    case 0n 1n+xp:
      {==}
    case 1n+yp 0n:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_eq(x, y), False{})
    case 1n+yp 1n+xp:
      hub_lt_neq(yp, xp, h)

# x < a and a <= b: x < b
def hub_lt_ge(+x: Nat, +a: Nat, +b: Nat, h: {Nat.is_lt(x, a) == True{} : Bool}, g: {Nat.is_ge(b, a) == True{} : Bool})
    -> {Nat.is_lt(x, b) == True{} : Bool}:
  match x a b:
    case _ 0n _:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_lt(x, b), True{})
    case 0n 1n+ap 0n:
      hub_absurd_eq(hub_ft_absurd(g), Bool, Nat.is_lt(x, b), True{})
    case 0n 1n+ap 1n+bp:
      {==}
    case 1n+xp 1n+ap 0n:
      hub_absurd_eq(hub_ft_absurd(g), Bool, Nat.is_lt(x, b), True{})
    case 1n+xp 1n+ap 1n+bp:
      hub_lt_ge(xp, ap, bp, h, g)

def hub_lt_succ(+n: Nat) -> {Nat.is_lt(n, 1n+n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      hub_lt_succ(p)

def hub_rec_op_id(+r: HM.MRec, op: HM.MsgOp, -rest: List<&2, Nat>)
    -> {HM.Msg.id(HM.MRec.msg(HM.rec_op(r, op))) <> rest == HM.Msg.id(HM.MRec.msg(r)) <> rest : List<&2, Nat>}:
  match r:
    case HM.MRec{m, st, se}:
      match op:
        case HM.OSt{x}:
          {==}
        case HM.OSettle{}:
          {==}

def hub_ids_map.pick(c: Bool, +h: HM.MRec, -t: List<&2, HM.MRec>, -id: Nat, +op: HM.MsgOp,
    +e: {Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id) == c : Bool}, ih: {HI.ids(HM.map_msg(t, id, op)) == HI.ids(t) : List<&2, Nat>})
    -> {HI.ids(Bool.pick(List<&2, HM.MRec>, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), HM.rec_op(h, op) <> t, h <> HM.map_msg(t, id, op)))
        == HI.ids(h <> t) : List<&2, Nat>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), True{}, e) : {HI.ids(Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(h, op) <> t, h <> HM.map_msg(t, id, op))) == HI.ids(h <> t) : List<&2, Nat>}
      hub_rec_op_id(h, op, HI.ids(t))
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), False{}, e) : {HI.ids(Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(h, op) <> t, h <> HM.map_msg(t, id, op))) == HI.ids(h <> t) : List<&2, Nat>}
      Equal.cong(List<&2, Nat>, List<&2, Nat>, xs => HM.Msg.id(HM.MRec.msg(h)) <> xs, HI.ids(HM.map_msg(t, id, op)), HI.ids(t), ih)

# updating one record keeps every id in place
def hub_ids_map(+xs: List<&2, HM.MRec>, +id: Nat, +op: HM.MsgOp) -> {HI.ids(HM.map_msg(xs, id, op)) == HI.ids(xs) : List<&2, Nat>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_ids_map.pick(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), h, t, id, op, {==}, hub_ids_map(t, id, op))

# settling a question keeps every id in place
def hub_settle_if_ids(ok: Bool, +xs: List<&2, HM.MRec>, +id: Nat) -> {HI.ids(HM.apply.settle.if(ok, xs, id)) == HI.ids(xs) : List<&2, Nat>}:
  match ok:
    case True{}:
      hub_ids_map(xs, id, HM.OSettle{})
    case False{}:
      {==}

def hub_settle_go_ids(o: Maybe<&2, HM.MRec>, +xs: List<&2, HM.MRec>, +id: Nat, +from: String)
    -> {HI.ids(HM.apply.settle.go(o, xs, id, from)) == HI.ids(xs) : List<&2, Nat>}:
  match o:
    case None{}:
      {==}
    case Some{+r}:
      hub_settle_if_ids(String.eq(HM.Msg.to(HM.MRec.msg(r)), from) || String.eq(from, HM.USER()), xs, id)

def hub_settle_ids(r: Maybe<&2, Nat>, +xs: List<&2, HM.MRec>, +from: String)
    -> {HI.ids(HM.apply.settle(r, xs, from)) == HI.ids(xs) : List<&2, Nat>}:
  match r:
    case None{}:
      {==}
    case Some{+id}:
      hub_settle_go_ids(HM.rec_of(xs, id), xs, id, from)

def hub_ids_snoc(+xs: List<&2, HM.MRec>, +r: HM.MRec)
    -> {HI.ids(HM.snoc(HM.MRec, xs, r)) == HM.snoc(Nat, HI.ids(xs), HM.Msg.id(HM.MRec.msg(r))) : List<&2, Nat>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, Nat>, List<&2, Nat>, ys => HM.Msg.id(HM.MRec.msg(h)) <> ys,
        HI.ids(HM.snoc(HM.MRec, t, r)), HM.snoc(Nat, HI.ids(t), HM.Msg.id(HM.MRec.msg(r))), hub_ids_snoc(t, r))

def hub_tf_absurd(e: {True{} == False{} : Bool}) -> Empty:
  %e : hub_bdisc(_)
  Unit{}

def hub_or_false_l(a: Bool, -b: Bool, e: {Bool.or(a, b) == False{} : Bool}) -> {a == False{} : Bool}:
  match a:
    case False{}:
      {==}
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, True{}, False{})

def hub_or_false_r(a: Bool, -b: Bool, e: {Bool.or(a, b) == False{} : Bool}) -> {b == False{} : Bool}:
  match a:
    case False{}:
      e
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, b, False{})

def hub_or_false(a: Bool, -b: Bool, ea: {a == False{} : Bool}, eb: {b == False{} : Bool}) -> {Bool.or(a, b) == False{} : Bool}:
  %Equal.sym(Bool, a, False{}, ea) : {Bool.or(_, b) == False{} : Bool}
  eb

# y is neither in t nor x: it is not in t ++ [x]
def hub_nat_in_snoc(+t: List<&2, Nat>, +x: Nat, +y: Nat, +a: {HM.nat_in(t, y) == False{} : Bool}, b: {Nat.is_eq(x, y) == False{} : Bool})
    -> {HM.nat_in(HM.snoc(Nat, t, x), y) == False{} : Bool}:
  match t:
    case Nil{}:
      hub_or_false(Nat.is_eq(x, y), False{}, b, {==})
    case Con{h, tt}:
      hub_or_false(Nat.is_eq(h, y), HM.nat_in(HM.snoc(Nat, tt, x), y),
        hub_or_false_l(Nat.is_eq(h, y), HM.nat_in(tt, y), a),
        hub_nat_in_snoc(tt, x, y, hub_or_false_r(Nat.is_eq(h, y), HM.nat_in(tt, y), a), b))

def hub_distinct_snoc(+ns: List<&2, Nat>, +x: Nat, d: HI.Distinct(ns), bl: HI.Below(ns, x)) -> HI.Distinct(HM.snoc(Nat, ns, x)):
  match ns:
    case Nil{}:
      HX.Both{{==}, Unit{}}
    case Con{y, t}:
      match d bl:
        case HX.Both{dy, dt} HX.Both{ly, bt}:
          HX.Both{hub_nat_in_snoc(t, x, y, dy, hub_lt_neq(y, x, ly)), hub_distinct_snoc(t, x, dt, bt)}

def hub_below_mono(+ns: List<&2, Nat>, +a: Nat, +b: Nat, +g: {Nat.is_ge(b, a) == True{} : Bool}, bl: HI.Below(ns, a)) -> HI.Below(ns, b):
  match ns:
    case Nil{}:
      Unit{}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          HX.Both{hub_lt_ge(y, a, b, ly, g), hub_below_mono(t, a, b, g, bt)}

def hub_below_snoc(+ns: List<&2, Nat>, +x: Nat, +n: Nat, bl: HI.Below(ns, n), lx: {Nat.is_lt(x, n) == True{} : Bool})
    -> HI.Below(HM.snoc(Nat, ns, x), n):
  match ns:
    case Nil{}:
      HX.Both{lx, Unit{}}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          HX.Both{ly, hub_below_snoc(t, x, n, bt, lx)}

def hub_distinct_tr(-x: List<&2, Nat>, -y: List<&2, Nat>, e: {x == y : List<&2, Nat>}, d: HI.Distinct(y)) -> HI.Distinct(x):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.Distinct(_)
  d

def hub_below_tr(-x: List<&2, Nat>, -y: List<&2, Nat>, -n: Nat, e: {x == y : List<&2, Nat>}, b: HI.Below(y, n)) -> HI.Below(x, n):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.Below(_, n)
  b

def hub_below_n(-ns: List<&2, Nat>, -a: Nat, -b: Nat, e: {a == b : Nat}, bl: HI.Below(ns, b)) -> HI.Below(ns, a):
  %Equal.sym(Nat, a, b, e) : HI.Below(ns, _)
  bl

# an update of one record keeps the state unique
def hub_unique_map(+ms: List<&2, HM.MRec>, +id: Nat, +op: HM.MsgOp, -nm: Nat,
    u: HX.Both<HI.Distinct(HI.ids(ms)), HI.Below(HI.ids(ms), nm)>)
    -> HX.Both<HI.Distinct(HI.ids(HM.map_msg(ms, id, op))), HI.Below(HI.ids(HM.map_msg(ms, id, op)), nm)>:
  match u:
    case HX.Both{d, b}:
      HX.Both{hub_distinct_tr(HI.ids(HM.map_msg(ms, id, op)), HI.ids(ms), hub_ids_map(ms, id, op), d),
        hub_below_tr(HI.ids(HM.map_msg(ms, id, op)), HI.ids(ms), nm, hub_ids_map(ms, id, op), b)}

# a new message with the next free id keeps the state unique
def hub_unique_sent(+ms: List<&2, HM.MRec>, +m: HM.Msg, +nm: Nat, k: {HM.Msg.id(m) == nm : Nat},
    u: HX.Both<HI.Distinct(HI.ids(ms)), HI.Below(HI.ids(ms), nm)>)
    -> HX.Both<HI.Distinct(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m)))),
               HI.Below(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m))),
                 Nat.max(Nat.max(nm, 1n+HM.Msg.id(m)), 1n+HM.Msg.thread(m)))>:
  match u:
    case HX.Both{+d, +b}:
      +xs = HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}})
      +a = Nat.max(nm, 1n+HM.Msg.id(m))
      +top = Nat.max(a, 1n+HM.Msg.thread(m))
      +e = Equal.trans(List<&2, Nat>, HI.ids(HM.apply.settle(HM.Msg.reply_to(m), xs, HM.Msg.from(m))), HI.ids(xs),
        HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)),
        hub_settle_ids(HM.Msg.reply_to(m), xs, HM.Msg.from(m)), hub_ids_snoc(ms, HM.MRec{m, HM.Queued{"new"}, False{}}))
      +lt = hub_lt_ge(HM.Msg.id(m), a, top,
        hub_lt_ge(HM.Msg.id(m), 1n+HM.Msg.id(m), a, hub_lt_succ(HM.Msg.id(m)), Nat.max_ge_r(nm, 1n+HM.Msg.id(m))),
        Nat.max_ge_l(a, 1n+HM.Msg.thread(m)))
      HX.Both{hub_distinct_tr(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), xs, HM.Msg.from(m))), HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)), e,
          hub_distinct_snoc(HI.ids(ms), HM.Msg.id(m), d, hub_below_n(HI.ids(ms), HM.Msg.id(m), nm, k, b))),
        hub_below_tr(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), xs, HM.Msg.from(m))), HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)), top, e,
          hub_below_snoc(HI.ids(ms), HM.Msg.id(m), top,
            hub_below_mono(HI.ids(ms), a, top, Nat.max_ge_l(a, 1n+HM.Msg.thread(m)),
              hub_below_mono(HI.ids(ms), nm, a, Nat.max_ge_l(nm, 1n+HM.Msg.id(m)), b)), lt))}

# every event with its journal invariant (a fresh id for a new message)
# keeps the state unique
def hub_apply_unique(s: HM.St, e: HM.Ev, u: HI.Unique(s), k: HX.FreshEv(s, e)) -> HI.Unique(HM.apply(s, e)):
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      match e:
        case HM.ETaskCreated{a, b, c, d, f}:
          u
        case HM.ELifecycle{a, b, c}:
          u
        case HM.EWorkspace{a, b}:
          u
        case HM.ESnapshot{a, b}:
          u
        case HM.EDeclared{a, b, c}:
          u
        case HM.EReported{a, b}:
          u
        case HM.EFileTouched{a, b}:
          u
        case HM.ERenamed{a, b}:
          u
        case HM.EMsgSent{m}:
          hub_unique_sent(ms, m, nm, k, u)
        case HM.EMsgState{id, x}:
          hub_unique_map(ms, id, HM.OSt{x}, nm, u)
        case HM.ESettled{id}:
          hub_unique_map(ms, id, HM.OSettle{}, nm, u)
        case HM.ECardOpened{c}:
          u
        case HM.ECardClosed{a, b}:
          u
        case HM.EMainNote{a, b}:
          u
        case HM.ENotesFlushed{}:
          u

def hub_apply_all_unique(+evs: List<&2, HM.Ev>, +s: HM.St, u: HI.Unique(s), f: HX.Fresh(evs, s)) -> HI.Unique(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      u
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          hub_apply_all_unique(t, HM.apply(s, e), hub_apply_unique(s, e, u, k), ft)

def Laws.init_unique(w):
  HX.Both{Unit{}, Unit{}}

def hub_unique_tr(-x: HM.St, -y: HM.St, e: {x == y : HM.St}, u: HI.Unique(y)) -> HI.Unique(x):
  %Equal.sym(HM.St, x, y, e) : HI.Unique(_)
  u

def Laws.msgs_unique(ins, h, u):
  hub_unique_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    hub_apply_all_unique(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))


# ---- L2: a message once sent is never removed (msgs_kept) ----

def hub_prefix_refl(+a: List<&2, Nat>) -> HI.IsPrefix(a, a):
  match a:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      HX.Both{{==}, hub_prefix_refl(t)}

def hub_prefix_snoc(+a: List<&2, Nat>, +b: List<&2, Nat>, -x: Nat, p: HI.IsPrefix(a, b)) -> HI.IsPrefix(a, HM.snoc(Nat, b, x)):
  match a b:
    case Nil{} _:
      Unit{}
    case Con{y, t} Nil{}:
      match p:
    case Con{y, t} Con{z, u}:
      match p:
        case HX.Both{e, pt}:
          HX.Both{e, hub_prefix_snoc(t, u, x, pt)}

def hub_prefix_trans(+a: List<&2, Nat>, +b: List<&2, Nat>, +c: List<&2, Nat>, p: HI.IsPrefix(a, b), q: HI.IsPrefix(b, c)) -> HI.IsPrefix(a, c):
  match a b c:
    case Nil{} _ _:
      Unit{}
    case Con{x, t} Nil{} _:
      match p:
    case Con{x, t} Con{y, u} Nil{}:
      match q:
    case Con{x, t} Con{y, u} Con{z, v}:
      match p q:
        case HX.Both{e1, pt} HX.Both{e2, qt}:
          HX.Both{Equal.trans(Nat, x, y, z, e1, e2), hub_prefix_trans(t, u, v, pt, qt)}

def hub_prefix_tr(-a: List<&2, Nat>, -x: List<&2, Nat>, -y: List<&2, Nat>, e: {x == y : List<&2, Nat>}, p: HI.IsPrefix(a, y)) -> HI.IsPrefix(a, x):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.IsPrefix(a, _)
  p

# every event keeps the messages, in order (a new one comes last)
def hub_apply_kept(s: HM.St, e: HM.Ev) -> HI.Kept(s, HM.apply(s, e)):
  match s:
    case HM.St{ag, od, +ms, ca, no, nm, nc}:
      match e:
        case HM.EMsgSent{+m}:
          hub_prefix_tr(HI.ids(ms),
            HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m))),
            HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)),
            Equal.trans(List<&2, Nat>,
              HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m))),
              HI.ids(HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}})), HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)),
              hub_settle_ids(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m)),
              hub_ids_snoc(ms, HM.MRec{m, HM.Queued{"new"}, False{}})),
            hub_prefix_snoc(HI.ids(ms), HI.ids(ms), HM.Msg.id(m), hub_prefix_refl(HI.ids(ms))))
        case HM.EMsgState{+id, +x}:
          hub_prefix_tr(HI.ids(ms), HI.ids(HM.map_msg(ms, id, HM.OSt{x})), HI.ids(ms), hub_ids_map(ms, id, HM.OSt{x}), hub_prefix_refl(HI.ids(ms)))
        case HM.ESettled{+id}:
          hub_prefix_tr(HI.ids(ms), HI.ids(HM.map_msg(ms, id, HM.OSettle{})), HI.ids(ms), hub_ids_map(ms, id, HM.OSettle{}), hub_prefix_refl(HI.ids(ms)))
        case HM.ETaskCreated{a, b, c, d, f}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ELifecycle{a, b, c}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EWorkspace{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ESnapshot{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EDeclared{a, b, c}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EReported{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EFileTouched{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ERenamed{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ECardOpened{c}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ECardClosed{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EMainNote{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ENotesFlushed{}:
          hub_prefix_refl(HI.ids(ms))

def hub_apply_all_kept(+evs: List<&2, HM.Ev>, +s: HM.St) -> HI.Kept(s, HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      hub_prefix_refl(HI.ids(HM.St.msgs(s)))
    case Con{e, t}:
      hub_prefix_trans(HI.ids(HM.St.msgs(s)), HI.ids(HM.St.msgs(HM.apply(s, e))), HI.ids(HM.St.msgs(HM.apply_all(t, HM.apply(s, e)))),
        hub_apply_kept(s, e), hub_apply_all_kept(t, HM.apply(s, e)))

def hub_kept_tr(-a: HM.St, -x: HM.St, -y: HM.St, e: {x == y : HM.St}, k: HI.Kept(a, y)) -> HI.Kept(a, x):
  %Equal.sym(HM.St, x, y, e) : HI.Kept(a, _)
  k

def Laws.msgs_kept(ins, h):
  hub_kept_tr(HC.Hub.d(h), HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h), hub_apply_all_kept(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)))

# ---- L3: delivered only from the queue (delivered_once) ----

def hub_deliv_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HM.St, fa: HX.Deliv(a, s),
    fb: HX.Deliv(b, HM.apply_all(a, s))) -> HX.Deliv(List.append(&2, HM.Ev, a, b), s):
  match a:
    case Nil{}:
      fb
    case Con{e, t}:
      match fa:
        case HX.Both{x, y}:
          HX.Both{x, hub_deliv_append(t, b, HM.apply(s, e), y, fb)}

def hub_deliv_tr(-evs: List<&2, HM.Ev>, -x: HM.St, -y: HM.St, e: {x == y : HM.St}, f: HX.Deliv(evs, x)) -> HX.Deliv(evs, y):
  %e : HX.Deliv(evs, _)
  f

def hub_commit_go_v(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> HX.Deliv(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)):
  match need:
    case Some{q}:
      Unit{}
    case None{}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          v

def hub_commit_v.cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> HX.Deliv(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)):
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go_v(h, HX.R.need(r), c, r)

def Laws.delivered_once(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      hub_deliv_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(h),
        hub_commit_v.cx(h, HC.step(h, i)),
        hub_deliv_tr(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))),
          HC.Hub.d(HC.Out.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)),
          hub_commit_ok(h, i), Laws.delivered_once(rest, HC.Out.hub(HC.commit(h, i)))))

# ---- L12: a stop leaves no queued message and no card of the task ----

def hub_d_out(-o: HM.St, cx: HX.Cx<o>, -x: J.Json) -> {HX.d(o, HX.out(o, cx, x)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_with_r(-o: HM.St, cx: HX.Cx<o>, -r: HX.R) -> {HX.d(o, HX.with_r(o, cx, r)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r0}:
      {==}

def hub_d_emit(-o: HM.St, cx: HX.Cx<o>, -e: HM.Ev, -k: HX.FreshEv(HX.d(o, cx), e), -kq: HX.QuietEv(e), -kv: HX.DelivEv(HX.d(o, cx), e))
    -> {HX.d(o, HX.emit(o, cx, e, k, kq, kv)) == HM.apply(HX.d(o, cx), e) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_set_rt(-o: HM.St, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.set_rt(o, cx, x)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_if_agent(-o: HM.St, known: Bool, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.if_agent(o, known, cx, x)) == HX.d(o, cx) : HM.St}:
  match known:
    case True{}:
      hub_d_set_rt(o, cx, x)
    case False{}:
      {==}

def hub_d_upd_rt(-o: HM.St, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.upd_rt(o, cx, x)) == HX.d(o, cx) : HM.St}:
  hub_d_if_agent(o, HC.has_agent(o, cx, HX.Rt.name(x)), cx, x)

def hub_d_reply_err(-o: HM.St, +cx: HX.Cx<o>, +t: Nat, +why: String) -> {HX.d(o, HC.reply_err(o, cx, t, why)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_reply_all(-o: HM.St, +ws: List<&2, HX.Waiter>, +cx: HX.Cx<o>, +why: String) -> {HX.d(o, HC.reply_all(o, ws, cx, why)) == HX.d(o, cx) : HM.St}:
  match ws:
    case Nil{}:
      {==}
    case Con{w, rest}:
      Equal.trans(HM.St, HX.d(o, HC.reply_all(o, rest, HC.reply_err(o, cx, HX.Waiter.token(w), why), why)),
        HX.d(o, HC.reply_err(o, cx, HX.Waiter.token(w), why)), HX.d(o, cx),
        hub_d_reply_all(o, rest, HC.reply_err(o, cx, HX.Waiter.token(w), why), why),
        hub_d_reply_err(o, cx, HX.Waiter.token(w), why))

def hub_d_fail_waiters(-o: HM.St, +cx: HX.Cx<o>, +a: String, +why: String) -> {HX.d(o, HC.fail_waiters(o, cx, a, why)) == HX.d(o, cx) : HM.St}:
  +ws = HX.R.waiters(HC.rr(o, cx))
  +cx2 = HC.set_r(o, cx, HX.R.with_waiters(HC.rr(o, cx), HC.waiters_not(ws, a)))
  +cx3 = HC.reply_all(o, HC.waiters_of(ws, a), cx2, why)
  Equal.trans(HM.St, HX.d(o, HC.upd_rt(o, cx3, HX.Rt.with_waiting(HC.rt(o, cx3, a), False{}))), HX.d(o, cx3), HX.d(o, cx),
    hub_d_upd_rt(o, cx3, HX.Rt.with_waiting(HC.rt(o, cx3, a), False{})),
    Equal.trans(HM.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx),
      hub_d_reply_all(o, HC.waiters_of(ws, a), cx2, why),
      hub_d_with_r(o, cx, HX.R.with_waiters(HC.rr(o, cx), HC.waiters_not(ws, a)))))

def hub_eq_refl(+n: Nat) -> {Nat.is_eq(n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      hub_eq_refl(p)

def hub_eq_sym(+a: Nat, +b: Nat) -> {Nat.is_eq(a, b) == Nat.is_eq(b, a) : Bool}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      hub_eq_sym(ap, bp)

# the records after the rejections of stop_task
def hub_rej(ids: List<&2, Nat>, +xs: List<&2, HM.MRec>) -> List<&2, HM.MRec>:
  match ids:
    case Nil{}:
      xs
    case Con{+id, +t}:
      hub_rej(t, HM.map_msg(xs, id, HM.OSt{HM.Rejected{"recipient_unavailable"}}))

def hub_msgs_state(s: HM.St, +id: Nat, +x: HM.MSt) -> {HM.St.msgs(HM.apply(s, HM.EMsgState{id, x})) == HM.map_msg(HM.St.msgs(s), id, HM.OSt{x}) : List<&2, HM.MRec>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      {==}

def hub_msgs_reject(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HM.St.msgs(HX.d(o, HC.reject_ids(o, ids, cx))) == hub_rej(ids, HM.St.msgs(HX.d(o, cx))) : List<&2, HM.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +cx2 = HC.emit(o, cx, HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.reject_ids(o, t, cx2))), hub_rej(t, HM.St.msgs(HX.d(o, cx2))),
        hub_rej(id <> t, HM.St.msgs(HX.d(o, cx))),
        hub_msgs_reject(o, t, cx2),
        Equal.cong(List<&2, HM.MRec>, List<&2, HM.MRec>, xs => hub_rej(t, xs), HM.St.msgs(HX.d(o, cx2)),
          HM.map_msg(HM.St.msgs(HX.d(o, cx)), id, HM.OSt{HM.Rejected{"recipient_unavailable"}}),
          Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}})),
            HM.map_msg(HM.St.msgs(HX.d(o, cx)), id, HM.OSt{HM.Rejected{"recipient_unavailable"}}),
            Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}}),
              hub_d_emit(o, cx, HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}, Unit{})),
            hub_msgs_state(HX.d(o, cx), id, HM.Rejected{"recipient_unavailable"}))))

def hub_rej_skip.go(+i: Nat, -ns: List<&2, Nat>, +h: HM.MRec, +t: List<&2, HM.MRec>,
    ef: {Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), i) == False{} : Bool},
    ih: {hub_rej(ns, h <> HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}})) == h <> hub_rej(ns, HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}})) : List<&2, HM.MRec>})
    -> {hub_rej(i <> ns, h <> t) == h <> hub_rej(i <> ns, t) : List<&2, HM.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), i), False{}, ef) : {hub_rej(ns, Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(h, HM.OSt{HM.Rejected{"recipient_unavailable"}}) <> t, h <> HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}}))) == h <> hub_rej(i <> ns, t) : List<&2, HM.MRec>}
  ih

# a record whose id is not rejected stays in place
def hub_rej_skip(+ids: List<&2, Nat>, +h: HM.MRec, +t: List<&2, HM.MRec>, +nf: {HM.nat_in(ids, HM.Msg.id(HM.MRec.msg(h))) == False{} : Bool})
    -> {hub_rej(ids, h <> t) == h <> hub_rej(ids, t) : List<&2, HM.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{i, ns}:
      hub_rej_skip.go(i, ns, h, t,
        Equal.trans(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), i), Nat.is_eq(i, HM.Msg.id(HM.MRec.msg(h))), False{},
          hub_eq_sym(HM.Msg.id(HM.MRec.msg(h)), i),
          hub_or_false_l(Nat.is_eq(i, HM.Msg.id(HM.MRec.msg(h))), HM.nat_in(ns, HM.Msg.id(HM.MRec.msg(h))), nf)),
        hub_rej_skip(ns, h, HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}}),
          hub_or_false_r(Nat.is_eq(i, HM.Msg.id(HM.MRec.msg(h))), HM.nat_in(ns, HM.Msg.id(HM.MRec.msg(h))), nf)))

def hub_q_sub.pick(c: Bool, +h: HM.MRec, -tt: List<&2, HM.MRec>, -n: String, +x: Nat,
    +e: {Bool.and(String.eq(HM.Msg.to(HM.MRec.msg(h)), n), HM.is_queued(HM.MRec.st(h))) == c : Bool},
    e1: {Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x) == False{} : Bool},
    ih: {HM.nat_in(HC.ids_of(HM.queued_in(tt, n)), x) == False{} : Bool})
    -> {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HM.Msg>, Bool.and(String.eq(HM.Msg.to(HM.MRec.msg(h)), n), HM.is_queued(HM.MRec.st(h))),
          HM.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Msg.to(HM.MRec.msg(h)), n), HM.is_queued(HM.MRec.st(h))), True{}, e) : {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, HM.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}
      hub_or_false(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x), HM.nat_in(HC.ids_of(HM.queued_in(tt, n)), x), e1, ih)
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Msg.to(HM.MRec.msg(h)), n), HM.is_queued(HM.MRec.st(h))), False{}, e) : {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, HM.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}
      ih

# the queued ids of a list are among its ids
def hub_q_sub(+t: List<&2, HM.MRec>, +x: Nat, +n: String, +nf: {HM.nat_in(HI.ids(t), x) == False{} : Bool})
    -> {HM.nat_in(HC.ids_of(HM.queued_in(t, n)), x) == False{} : Bool}:
  match t:
    case Nil{}:
      {==}
    case Con{h, tt}:
      hub_q_sub.pick(Bool.and(String.eq(HM.Msg.to(HM.MRec.msg(h)), n), HM.is_queued(HM.MRec.st(h))), h, tt, n, x, {==},
        hub_or_false_l(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x), HM.nat_in(HI.ids(tt), x), nf),
        hub_q_sub(tt, x, n, hub_or_false_r(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x), HM.nat_in(HI.ids(tt), x), nf)))

def hub_and_left(a: Bool, -b: Bool, e: {Bool.and(a, b) == True{} : Bool}) -> {a == True{} : Bool}:
  match a:
    case True{}:
      {==}
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, False{}, True{})

def hub_K.pick(c: Bool, +m: HM.Msg, +st: HM.MSt, +se: Bool, +t: List<&2, HM.MRec>, +n: String,
    +e: {Bool.and(String.eq(HM.Msg.to(m), n), HM.is_queued(st)) == c : Bool}, +dh: {HM.nat_in(HI.ids(t), HM.Msg.id(m)) == False{} : Bool},
    ih: {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n) == Nil{} : List<&2, HM.Msg>})
    -> {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HM.Msg>, Bool.and(String.eq(HM.Msg.to(m), n), HM.is_queued(st)), m <> HM.queued_in(t, n), HM.queued_in(t, n))), HM.MRec{m, st, se} <> t), n) == Nil{} : List<&2, HM.Msg>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Msg.to(m), n), HM.is_queued(st)), True{}, e) : {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(t, n), HM.queued_in(t, n))), HM.MRec{m, st, se} <> t), n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(m), HM.Msg.id(m)), True{}, hub_eq_refl(HM.Msg.id(m))) : {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(HM.MRec{m, st, se}, HM.OSt{HM.Rejected{"recipient_unavailable"}}) <> t, HM.MRec{m, st, se} <> HM.map_msg(t, HM.Msg.id(m), HM.OSt{HM.Rejected{"recipient_unavailable"}}))), n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(List<&2, HM.MRec>, hub_rej(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, HM.Rejected{"recipient_unavailable"}, se} <> t), HM.MRec{m, HM.Rejected{"recipient_unavailable"}, se} <> hub_rej(HC.ids_of(HM.queued_in(t, n)), t), hub_rej_skip(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, HM.Rejected{"recipient_unavailable"}, se}, t, hub_q_sub(t, HM.Msg.id(m), n, dh))) : {HM.queued_in(_, n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(Bool, String.eq(HM.Msg.to(m), n), True{}, hub_and_left(String.eq(HM.Msg.to(m), n), HM.is_queued(st), e)) : {Bool.pick(List<&2, HM.Msg>, Bool.and(_, False{}), m <> HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n), HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n)) == Nil{} : List<&2, HM.Msg>}
      ih
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Msg.to(m), n), HM.is_queued(st)), False{}, e) : {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(t, n), HM.queued_in(t, n))), HM.MRec{m, st, se} <> t), n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(List<&2, HM.MRec>, hub_rej(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, st, se} <> t), HM.MRec{m, st, se} <> hub_rej(HC.ids_of(HM.queued_in(t, n)), t), hub_rej_skip(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, st, se}, t, hub_q_sub(t, HM.Msg.id(m), n, dh))) : {HM.queued_in(_, n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(Bool, Bool.and(String.eq(HM.Msg.to(m), n), HM.is_queued(st)), False{}, e) : {Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n), HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n)) == Nil{} : List<&2, HM.Msg>}
      ih

# rejecting every message queued for `n` leaves none queued for it
def hub_K(+xs: List<&2, HM.MRec>, +n: String, d: HI.Distinct(HI.ids(xs))) -> {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(xs, n)), xs), n) == Nil{} : List<&2, HM.Msg>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match h:
        case HM.MRec{m, st, se}:
          match d:
            case HX.Both{dh, dt}:
              hub_K.pick(Bool.and(String.eq(HM.Msg.to(m), n), HM.is_queued(st)), m, st, se, t, n, {==}, dh, hub_K(t, n, dt))

def hub_d_hout(-o: HM.St, +cx: HX.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> {HX.d(o, HC.out(o, cx, kvs)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_msgs_closed(s: HM.St, +id: Nat, +res: String) -> {HM.St.msgs(HM.apply(s, HM.ECardClosed{id, res})) == HM.St.msgs(s) : List<&2, HM.MRec>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      {==}

def hub_msgs_life(s: HM.St, +n: String, +l: HM.Life, +r: Maybe<&2, String>) -> {HM.St.msgs(HM.apply(s, HM.ELifecycle{n, l, r})) == HM.St.msgs(s) : List<&2, HM.MRec>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      {==}

def hub_msgs_eq(-o: HM.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HM.St}) -> {HM.St.msgs(HX.d(o, a)) == HM.St.msgs(HX.d(o, b)) : List<&2, HM.MRec>}:
  Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, a), HX.d(o, b), e)

def hub_d_line(-o: HM.St, +cx: HX.Cx<o>, +a: String, +k: String, +t: String) -> {HX.d(o, HC.line(o, cx, a, k, t)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

# closing cards keeps the messages
def hub_msgs_close_ids(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {HM.St.msgs(HX.d(o, HC.close_ids(o, ids, cx, res))) == HM.St.msgs(HX.d(o, cx)) : List<&2, HM.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +cx2 = HC.emit(o, cx, HM.ECardClosed{id, res}, Unit{}, Unit{}, Unit{})
      +cx3 = HC.line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))
      Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.close_ids(o, rest, cx3, res))), HM.St.msgs(HX.d(o, cx3)), HM.St.msgs(HX.d(o, cx)),
        hub_msgs_close_ids(o, rest, cx3, res),
        Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, cx3)), HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HX.d(o, cx)),
          hub_msgs_eq(o, cx3, cx2, hub_d_line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))),
          Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HM.apply(HX.d(o, cx), HM.ECardClosed{id, res})), HM.St.msgs(HX.d(o, cx)),
            Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), HM.ECardClosed{id, res}),
              hub_d_emit(o, cx, HM.ECardClosed{id, res}, Unit{}, Unit{}, Unit{})),
            hub_msgs_closed(HX.d(o, cx), id, res))))

def hub_msgs_tr(-a: List<&2, HM.MRec>, -b: List<&2, HM.MRec>, -n: String, e: {a == b : List<&2, HM.MRec>},
    k: {HM.queued_in(b, n) == Nil{} : List<&2, HM.Msg>}) -> {HM.queued_in(a, n) == Nil{} : List<&2, HM.Msg>}:
  %Equal.sym(List<&2, HM.MRec>, a, b, e) : {HM.queued_in(_, n) == Nil{} : List<&2, HM.Msg>}
  k

def Laws.stop_leaves_no_queue(o, cx, n, life, why, u):
  +cx1 = HC.out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
  +cx2 = HC.fail_waiters(o, cx1, n, "stopped")
  +r = HC.rr(o, cx2)
  +cx3 = HC.set_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n)))
  +x4 = HX.Rt.with_turn(HX.Rt.with_run(HC.rt(o, cx3, n), HM.RDown{}), None{})
  +cx4 = HC.upd_rt(o, cx3, x4)
  +ev = {HM.ELifecycle{n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})} : HM.Ev}
  +cx5 = HC.emit(o, cx4, ev, Unit{}, Unit{}, Unit{})
  +m5 = HM.St.msgs(HX.d(o, cx5))
  +ids = HC.ids_of(HM.queued_in(m5, n))
  +cx6 = HC.reject_ids(o, ids, cx5)
  +g = HC.stop_task.go(o, cx, n, life, why)
  # d is unchanged up to cx4
  +e4 = Equal.trans(HM.St, HX.d(o, cx4), HX.d(o, cx3), HX.d(o, cx), hub_d_upd_rt(o, cx3, x4),
    Equal.trans(HM.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx), hub_d_with_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n))),
      Equal.trans(HM.St, HX.d(o, cx2), HX.d(o, cx1), HX.d(o, cx), hub_d_fail_waiters(o, cx1, n, "stopped"),
        hub_d_hout(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})]))))
  # the messages of cx5 are those of cx
  +e5 = Equal.trans(List<&2, HM.MRec>, m5, HM.St.msgs(HM.apply(HX.d(o, cx4), ev)), HM.St.msgs(HX.d(o, cx)),
    Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, cx5), HM.apply(HX.d(o, cx4), ev), hub_d_emit(o, cx4, ev, Unit{}, Unit{}, Unit{})),
    Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HM.apply(HX.d(o, cx4), ev)), HM.St.msgs(HX.d(o, cx4)), HM.St.msgs(HX.d(o, cx)),
      hub_msgs_life(HX.d(o, cx4), n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})),
      hub_msgs_eq(o, cx4, cx, e4)))
  # the messages at the end: those of cx5 with the queued ones rejected
  +ef = Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.msgs(HX.d(o, g)), hub_rej(ids, m5),
    hub_msgs_eq(o, HC.stop_task(o, cx, n, life, why), g, hub_d_with_r(o, g, HX.R.with_waiters(HC.rr(o, g), HC.waiters_not(HX.R.waiters(HC.rr(o, cx)), n)))),
    Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, g)), HM.St.msgs(HX.d(o, cx6)), hub_rej(ids, m5),
      hub_msgs_close_ids(o, HC.card_ids(HM.St.cards(HX.d(o, cx6)), n, Nil{}), cx6, "task stopped"),
      hub_msgs_reject(o, ids, cx5)))
  hub_msgs_tr(HM.St.msgs(HX.d(o, HC.stop_task(o, cx, n, life, why))), hub_rej(ids, m5), n, ef,
    hub_K(m5, n, hub_distinct_tr(HI.ids(m5), HI.ids(HM.St.msgs(HX.d(o, cx))),
      Equal.cong(List<&2, HM.MRec>, List<&2, Nat>, xs => HI.ids(xs), m5, HM.St.msgs(HX.d(o, cx)), e5),
      hub_fst(HI.Distinct(HI.ids(HM.St.msgs(HX.d(o, cx)))), HI.Below(HI.ids(HM.St.msgs(HX.d(o, cx))), HM.St.next_msg(HX.d(o, cx))), u))))

# the cards after the closes of stop_task
def hub_dropc(ids: List<&2, Nat>, +cs: List<&2, HM.Card>) -> List<&2, HM.Card>:
  match ids:
    case Nil{}:
      cs
    case Con{+id, +t}:
      hub_dropc(t, HM.drop_card(cs, id))

def hub_dropc_skip.go(+i: Nat, -ns: List<&2, Nat>, +h: HM.Card, +t: List<&2, HM.Card>,
    ef: {Nat.is_eq(HM.Card.id(h), i) == False{} : Bool},
    ih: {hub_dropc(ns, h <> HM.drop_card(t, i)) == h <> hub_dropc(ns, HM.drop_card(t, i)) : List<&2, HM.Card>})
    -> {hub_dropc(i <> ns, h <> t) == h <> hub_dropc(i <> ns, t) : List<&2, HM.Card>}:
  %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), i), False{}, ef) : {hub_dropc(ns, Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, i))) == h <> hub_dropc(i <> ns, t) : List<&2, HM.Card>}
  ih

def hub_dropc_skip(+ids: List<&2, Nat>, +h: HM.Card, +t: List<&2, HM.Card>, +nf: {HM.nat_in(ids, HM.Card.id(h)) == False{} : Bool})
    -> {hub_dropc(ids, h <> t) == h <> hub_dropc(ids, t) : List<&2, HM.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{i, ns}:
      hub_dropc_skip.go(i, ns, h, t,
        Equal.trans(Bool, Nat.is_eq(HM.Card.id(h), i), Nat.is_eq(i, HM.Card.id(h)), False{},
          hub_eq_sym(HM.Card.id(h), i),
          hub_or_false_l(Nat.is_eq(i, HM.Card.id(h)), HM.nat_in(ns, HM.Card.id(h)), nf)),
        hub_dropc_skip(ns, h, HM.drop_card(t, i), hub_or_false_r(Nat.is_eq(i, HM.Card.id(h)), HM.nat_in(ns, HM.Card.id(h)), nf)))

def hub_c_sub.pick(c: Bool, +h: HM.Card, -t: List<&2, HM.Card>, -n: String, +x: Nat,
    +e: {Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))) == c : Bool}, e1: {Nat.is_eq(HM.Card.id(h), x) == False{} : Bool},
    ih: {HM.nat_in(HC.card_ids(t, n, Nil{}), x) == False{} : Bool})
    -> {HM.nat_in(Bool.pick(List<&2, Nat>, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), True{}, e) : {HM.nat_in(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}
      hub_or_false(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HC.card_ids(t, n, Nil{}), x), e1, ih)
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), False{}, e) : {HM.nat_in(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}
      ih

def hub_c_sub(+t: List<&2, HM.Card>, +x: Nat, +n: String, +nf: {HM.nat_in(HI.card_ids(t), x) == False{} : Bool})
    -> {HM.nat_in(HC.card_ids(t, n, Nil{}), x) == False{} : Bool}:
  match t:
    case Nil{}:
      {==}
    case Con{h, tt}:
      hub_c_sub.pick(Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), h, tt, n, x, {==},
        hub_or_false_l(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(tt), x), nf),
        hub_c_sub(tt, x, n, hub_or_false_r(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(tt), x), nf)))

def hub_KC.pick(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, +n: String,
    +e: {Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))) == c : Bool}, +dh: {HM.nat_in(HI.card_ids(t), HM.Card.id(h)) == False{} : Bool},
    ih: {HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{}) == Nil{} : List<&2, Nat>})
    -> {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), True{}, e) : {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), HM.Card.id(h)), True{}, hub_eq_refl(HM.Card.id(h))) : {HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, HM.Card.id(h)))), n, Nil{}) == Nil{} : List<&2, Nat>}
      ih
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), False{}, e) : {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(List<&2, HM.Card>, hub_dropc(HC.card_ids(t, n, Nil{}), h <> t), h <> hub_dropc(HC.card_ids(t, n, Nil{}), t), hub_dropc_skip(HC.card_ids(t, n, Nil{}), h, t, hub_c_sub(t, HM.Card.id(h), n, dh))) : {HC.card_ids(_, n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), False{}, e) : {Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{}), HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{})) == Nil{} : List<&2, Nat>}
      ih

# closing every card of `n` leaves none of its cards
def hub_KC(+cs: List<&2, HM.Card>, +n: String, d: HI.Distinct(HI.card_ids(cs))) -> {HC.card_ids(hub_dropc(HC.card_ids(cs, n, Nil{}), cs), n, Nil{}) == Nil{} : List<&2, Nat>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match d:
        case HX.Both{dh, dt}:
          hub_KC.pick(Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), h, t, n, {==}, dh, hub_KC(t, n, dt))

def hub_cards_eq(-o: HM.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HM.St}) -> {HM.St.cards(HX.d(o, a)) == HM.St.cards(HX.d(o, b)) : List<&2, HM.Card>}:
  Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, a), HX.d(o, b), e)

def hub_cards_closed(s: HM.St, +id: Nat, +res: String) -> {HM.St.cards(HM.apply(s, HM.ECardClosed{id, res})) == HM.drop_card(HM.St.cards(s), id) : List<&2, HM.Card>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      {==}

def hub_cards_life(s: HM.St, +n: String, +l: HM.Life, +r: Maybe<&2, String>) -> {HM.St.cards(HM.apply(s, HM.ELifecycle{n, l, r})) == HM.St.cards(s) : List<&2, HM.Card>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      {==}

def hub_cards_state(s: HM.St, +id: Nat, +x: HM.MSt) -> {HM.St.cards(HM.apply(s, HM.EMsgState{id, x})) == HM.St.cards(s) : List<&2, HM.Card>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      {==}

# rejecting messages keeps the cards
def hub_cards_reject(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HM.St.cards(HX.d(o, HC.reject_ids(o, ids, cx))) == HM.St.cards(HX.d(o, cx)) : List<&2, HM.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.reject_ids(o, t, cx2))), HM.St.cards(HX.d(o, cx2)), HM.St.cards(HX.d(o, cx)),
        hub_cards_reject(o, t, cx2),
        Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx2)), HM.St.cards(HM.apply(HX.d(o, cx), ev)), HM.St.cards(HX.d(o, cx)),
          Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_cards_state(HX.d(o, cx), id, HM.Rejected{"recipient_unavailable"})))

# closing cards drops them, one by one
def hub_cards_close_ids(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {HM.St.cards(HX.d(o, HC.close_ids(o, ids, cx, res))) == hub_dropc(ids, HM.St.cards(HX.d(o, cx))) : List<&2, HM.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +ev = {HM.ECardClosed{id, res} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      +cx3 = HC.line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))
      Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.close_ids(o, rest, cx3, res))), hub_dropc(rest, HM.St.cards(HX.d(o, cx3))),
        hub_dropc(id <> rest, HM.St.cards(HX.d(o, cx))),
        hub_cards_close_ids(o, rest, cx3, res),
        Equal.cong(List<&2, HM.Card>, List<&2, HM.Card>, cs => hub_dropc(rest, cs), HM.St.cards(HX.d(o, cx3)), HM.drop_card(HM.St.cards(HX.d(o, cx)), id),
          Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx3)), HM.St.cards(HX.d(o, cx2)), HM.drop_card(HM.St.cards(HX.d(o, cx)), id),
            hub_cards_eq(o, cx3, cx2, hub_d_line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))),
            Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx2)), HM.St.cards(HM.apply(HX.d(o, cx), ev)), HM.drop_card(HM.St.cards(HX.d(o, cx)), id),
              Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
              hub_cards_closed(HX.d(o, cx), id, res)))))

def hub_cids_tr(-a: List<&2, HM.Card>, -b: List<&2, HM.Card>, -n: String, e: {a == b : List<&2, HM.Card>},
    k: {HC.card_ids(b, n, Nil{}) == Nil{} : List<&2, Nat>}) -> {HC.card_ids(a, n, Nil{}) == Nil{} : List<&2, Nat>}:
  %Equal.sym(List<&2, HM.Card>, a, b, e) : {HC.card_ids(_, n, Nil{}) == Nil{} : List<&2, Nat>}
  k

def Laws.stop_leaves_no_card(o, cx, n, life, why, u):
  +cx1 = HC.out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
  +cx2 = HC.fail_waiters(o, cx1, n, "stopped")
  +r = HC.rr(o, cx2)
  +cx3 = HC.set_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n)))
  +x4 = HX.Rt.with_turn(HX.Rt.with_run(HC.rt(o, cx3, n), HM.RDown{}), None{})
  +cx4 = HC.upd_rt(o, cx3, x4)
  +ev = {HM.ELifecycle{n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})} : HM.Ev}
  +cx5 = HC.emit(o, cx4, ev, Unit{}, Unit{}, Unit{})
  +ids = HC.ids_of(HM.queued_in(HM.St.msgs(HX.d(o, cx5)), n))
  +cx6 = HC.reject_ids(o, ids, cx5)
  +c6 = HM.St.cards(HX.d(o, cx6))
  +g = HC.stop_task.go(o, cx, n, life, why)
  +e4 = Equal.trans(HM.St, HX.d(o, cx4), HX.d(o, cx3), HX.d(o, cx), hub_d_upd_rt(o, cx3, x4),
    Equal.trans(HM.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx), hub_d_with_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n))),
      Equal.trans(HM.St, HX.d(o, cx2), HX.d(o, cx1), HX.d(o, cx), hub_d_fail_waiters(o, cx1, n, "stopped"),
        hub_d_hout(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})]))))
  # the cards of cx6 are those of cx
  +e6 = Equal.trans(List<&2, HM.Card>, c6, HM.St.cards(HX.d(o, cx5)), HM.St.cards(HX.d(o, cx)),
    hub_cards_reject(o, ids, cx5),
    Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx5)), HM.St.cards(HM.apply(HX.d(o, cx4), ev)), HM.St.cards(HX.d(o, cx)),
      Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, cx5), HM.apply(HX.d(o, cx4), ev), hub_d_emit(o, cx4, ev, Unit{}, Unit{}, Unit{})),
      Equal.trans(List<&2, HM.Card>, HM.St.cards(HM.apply(HX.d(o, cx4), ev)), HM.St.cards(HX.d(o, cx4)), HM.St.cards(HX.d(o, cx)),
        hub_cards_life(HX.d(o, cx4), n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})),
        hub_cards_eq(o, cx4, cx, e4))))
  # the cards at the end: those of cx6 without the task's
  +ef = Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.cards(HX.d(o, g)),
    hub_dropc(HC.card_ids(c6, n, Nil{}), c6),
    hub_cards_eq(o, HC.stop_task(o, cx, n, life, why), g, hub_d_with_r(o, g, HX.R.with_waiters(HC.rr(o, g), HC.waiters_not(HX.R.waiters(HC.rr(o, cx)), n)))),
    hub_cards_close_ids(o, HC.card_ids(c6, n, Nil{}), cx6, "task stopped"))
  hub_cids_tr(HM.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), hub_dropc(HC.card_ids(c6, n, Nil{}), c6), n, ef,
    hub_KC(c6, n, hub_distinct_tr(HI.card_ids(c6), HI.card_ids(HM.St.cards(HX.d(o, cx))),
      Equal.cong(List<&2, HM.Card>, List<&2, Nat>, cs => HI.card_ids(cs), c6, HM.St.cards(HX.d(o, cx)), e6),
      hub_fst(HI.Distinct(HI.card_ids(HM.St.cards(HX.d(o, cx)))), HI.Below(HI.card_ids(HM.St.cards(HX.d(o, cx))), HM.St.next_card(HX.d(o, cx))), u))))


# ---- L12 support: no two open cards share an id (cards_unique) ----

def hub_below_notin(+ns: List<&2, Nat>, +x: Nat, bl: HI.Below(ns, x)) -> {HM.nat_in(ns, x) == False{} : Bool}:
  match ns:
    case Nil{}:
      {==}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          hub_or_false(Nat.is_eq(y, x), HM.nat_in(t, x),
            Equal.trans(Bool, Nat.is_eq(y, x), Nat.is_eq(x, y), False{}, hub_eq_sym(y, x), hub_lt_neq(y, x, ly)),
            hub_below_notin(t, x, bt))

def hub_drop_absent.go(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, -id: Nat, +e: {Nat.is_eq(HM.Card.id(h), id) == c : Bool},
    ec: {c == False{} : Bool}, ih: {HM.drop_card(t, id) == t : List<&2, HM.Card>}) -> {HM.drop_card(h <> t, id) == h <> t : List<&2, HM.Card>}:
  match c:
    case True{}:
      hub_absurd_eq(hub_tf_absurd(ec), List<&2, HM.Card>, HM.drop_card(h <> t, id), h <> t)
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), False{}, e) : {Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id)) == h <> t : List<&2, HM.Card>}
      Equal.cong(List<&2, HM.Card>, List<&2, HM.Card>, xs => h <> xs, HM.drop_card(t, id), t, ih)

# dropping an id no card has changes nothing
def hub_drop_absent(+cs: List<&2, HM.Card>, +id: Nat, +nf: {HM.nat_in(HI.card_ids(cs), id) == False{} : Bool}) -> {HM.drop_card(cs, id) == cs : List<&2, HM.Card>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_drop_absent.go(Nat.is_eq(HM.Card.id(h), id), h, t, id, {==},
        hub_or_false_l(Nat.is_eq(HM.Card.id(h), id), HM.nat_in(HI.card_ids(t), id), nf),
        hub_drop_absent(t, id, hub_or_false_r(Nat.is_eq(HM.Card.id(h), id), HM.nat_in(HI.card_ids(t), id), nf)))

def hub_cids_snoc(+cs: List<&2, HM.Card>, +c: HM.Card) -> {HI.card_ids(HM.snoc(HM.Card, cs, c)) == HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)) : List<&2, Nat>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, Nat>, List<&2, Nat>, ys => HM.Card.id(h) <> ys, HI.card_ids(HM.snoc(HM.Card, t, c)), HM.snoc(Nat, HI.card_ids(t), HM.Card.id(c)), hub_cids_snoc(t, c))

def hub_drop_notin.go(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, -id: Nat, +x: Nat, +e: {Nat.is_eq(HM.Card.id(h), id) == c : Bool},
    +nf: {HM.nat_in(HI.card_ids(h <> t), x) == False{} : Bool}, ih: {HM.nat_in(HI.card_ids(HM.drop_card(t, id)), x) == False{} : Bool})
    -> {HM.nat_in(HI.card_ids(HM.drop_card(h <> t, id)), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), True{}, e) : {HM.nat_in(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), x) == False{} : Bool}
      hub_or_false_r(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf)
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), False{}, e) : {HM.nat_in(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), x) == False{} : Bool}
      hub_or_false(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(HM.drop_card(t, id)), x),
        hub_or_false_l(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf), ih)

# dropping a card never brings back an id
def hub_drop_notin(+cs: List<&2, HM.Card>, +id: Nat, +x: Nat, +nf: {HM.nat_in(HI.card_ids(cs), x) == False{} : Bool})
    -> {HM.nat_in(HI.card_ids(HM.drop_card(cs, id)), x) == False{} : Bool}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_drop_notin.go(Nat.is_eq(HM.Card.id(h), id), h, t, id, x, {==}, nf,
        hub_drop_notin(t, id, x, hub_or_false_r(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf)))

def hub_ucards_cons(+h: HM.Card, -ys: List<&2, HM.Card>, -n: Nat, nf: {HM.nat_in(HI.card_ids(ys), HM.Card.id(h)) == False{} : Bool},
    lh: {Nat.is_lt(HM.Card.id(h), n) == True{} : Bool}, u: HX.Both<HI.Distinct(HI.card_ids(ys)), HI.Below(HI.card_ids(ys), n)>)
    -> HX.Both<HI.Distinct(HI.card_ids(h <> ys)), HI.Below(HI.card_ids(h <> ys), n)>:
  match u:
    case HX.Both{d2, b2}:
      HX.Both{HX.Both{nf, d2}, HX.Both{lh, b2}}

def hub_ucards_drop.go(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, +id: Nat, -n: Nat, +e: {Nat.is_eq(HM.Card.id(h), id) == c : Bool},
    +dh: {HM.nat_in(HI.card_ids(t), HM.Card.id(h)) == False{} : Bool}, +lh: {Nat.is_lt(HM.Card.id(h), n) == True{} : Bool},
    +ut: HX.Both<HI.Distinct(HI.card_ids(t)), HI.Below(HI.card_ids(t), n)>, ih: HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(t, id))), HI.Below(HI.card_ids(HM.drop_card(t, id)), n)>) -> HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(h <> t, id))), HI.Below(HI.card_ids(HM.drop_card(h <> t, id)), n)>:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), True{}, e) : HX.Both<HI.Distinct(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id)))), HI.Below(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), n)>
      ut
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), False{}, e) : HX.Both<HI.Distinct(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id)))), HI.Below(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), n)>
      hub_ucards_cons(h, HM.drop_card(t, id), n, hub_drop_notin(t, id, HM.Card.id(h), dh), lh, ih)

# closing a card keeps the open cards unique
def hub_ucards_drop(+cs: List<&2, HM.Card>, +id: Nat, +n: Nat, u: HX.Both<HI.Distinct(HI.card_ids(cs)), HI.Below(HI.card_ids(cs), n)>) -> HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(cs, id))), HI.Below(HI.card_ids(HM.drop_card(cs, id)), n)>:
  match cs:
    case Nil{}:
      u
    case Con{h, t}:
      match u:
        case HX.Both{d, b}:
          match d b:
            case HX.Both{+dh, +dt} HX.Both{+lh, +bt}:
              hub_ucards_drop.go(Nat.is_eq(HM.Card.id(h), id), h, t, id, n, {==}, dh, lh, HX.Both{dt, bt}, hub_ucards_drop(t, id, n, HX.Both{dt, bt}))

# opening a card with the next free id keeps the open cards unique
def hub_ucards_open(+cs: List<&2, HM.Card>, +c: HM.Card, +nc: Nat, +k: {HM.Card.id(c) == nc : Nat}, u: HX.Both<HI.Distinct(HI.card_ids(cs)), HI.Below(HI.card_ids(cs), nc)>)
    -> HX.Both<HI.Distinct(HI.card_ids(HM.put_card(cs, c))), HI.Below(HI.card_ids(HM.put_card(cs, c)), Nat.max(nc, 1n+HM.Card.id(c)))>:
  match u:
    case HX.Both{+d, +b}:
      +top = Nat.max(nc, 1n+HM.Card.id(c))
      +absent = hub_drop_absent(cs, HM.Card.id(c), hub_below_notin(HI.card_ids(cs), HM.Card.id(c), hub_below_n(HI.card_ids(cs), HM.Card.id(c), nc, k, b)))
      +e = Equal.trans(List<&2, Nat>, HI.card_ids(HM.put_card(cs, c)), HI.card_ids(HM.snoc(HM.Card, cs, c)), HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)),
        Equal.cong(List<&2, HM.Card>, List<&2, Nat>, xs => HI.card_ids(HM.snoc(HM.Card, xs, c)), HM.drop_card(cs, HM.Card.id(c)), cs, absent),
        hub_cids_snoc(cs, c))
      HX.Both{hub_distinct_tr(HI.card_ids(HM.put_card(cs, c)), HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)), e,
          hub_distinct_snoc(HI.card_ids(cs), HM.Card.id(c), d, hub_below_n(HI.card_ids(cs), HM.Card.id(c), nc, k, b))),
        hub_below_tr(HI.card_ids(HM.put_card(cs, c)), HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)), top, e,
          hub_below_snoc(HI.card_ids(cs), HM.Card.id(c), top, hub_below_mono(HI.card_ids(cs), nc, top, Nat.max_ge_l(nc, 1n+HM.Card.id(c)), b),
            hub_lt_ge(HM.Card.id(c), 1n+HM.Card.id(c), top, hub_lt_succ(HM.Card.id(c)), Nat.max_ge_r(nc, 1n+HM.Card.id(c)))))}

def hub_apply_ucards(s: HM.St, e: HM.Ev, u: HI.UniqueCards(s), k: HX.FreshEv(s, e)) -> HI.UniqueCards(HM.apply(s, e)):
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc}:
      match e:
        case HM.ECardOpened{c}:
          hub_ucards_open(ca, c, nc, k, u)
        case HM.ECardClosed{id, res}:
          hub_ucards_drop(ca, id, nc, u)
        case HM.ETaskCreated{a, b, c, d, f}:
          u
        case HM.ELifecycle{a, b, c}:
          u
        case HM.EWorkspace{a, b}:
          u
        case HM.ESnapshot{a, b}:
          u
        case HM.EDeclared{a, b, c}:
          u
        case HM.EReported{a, b}:
          u
        case HM.EFileTouched{a, b}:
          u
        case HM.ERenamed{a, b}:
          u
        case HM.EMsgSent{m}:
          u
        case HM.EMsgState{a, b}:
          u
        case HM.ESettled{a}:
          u
        case HM.EMainNote{a, b}:
          u
        case HM.ENotesFlushed{}:
          u

def hub_apply_all_ucards(+evs: List<&2, HM.Ev>, +s: HM.St, u: HI.UniqueCards(s), f: HX.Fresh(evs, s)) -> HI.UniqueCards(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      u
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          hub_apply_all_ucards(t, HM.apply(s, e), hub_apply_ucards(s, e, u, k), ft)

def hub_ucards_tr(-x: HM.St, -y: HM.St, e: {x == y : HM.St}, u: HI.UniqueCards(y)) -> HI.UniqueCards(x):
  %Equal.sym(HM.St, x, y, e) : HI.UniqueCards(_)
  u

def Laws.init_cards_unique(w):
  HX.Both{Unit{}, Unit{}}

def Laws.cards_unique(ins, h, u):
  hub_ucards_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    hub_apply_all_ucards(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))
