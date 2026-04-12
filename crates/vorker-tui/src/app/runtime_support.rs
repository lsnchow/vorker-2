use super::*;

pub(crate) fn persist_dirty_thread(
    app: &mut App,
    thread_store: &mut ThreadStore,
    session_event_store: &SessionEventStore,
) -> io::Result<()> {
    if let Some(thread) = app.take_dirty_thread() {
        let previous = thread_store.thread(&thread.id);
        let events = derive_thread_events(previous.as_ref(), &thread);
        thread_store.upsert(thread.clone())?;
        session_event_store.append(&thread.id, &events)?;
    }
    Ok(())
}

pub(crate) fn prompt_history_for_app(store: &PromptHistoryStore) -> Vec<String> {
    let mut prompts = store
        .recent(50)
        .into_iter()
        .map(|entry| entry.text)
        .collect::<Vec<_>>();
    prompts.reverse();
    prompts
}

pub(crate) fn refresh_skill_state(
    app: &mut App,
    cwd: &Path,
    store: &crate::SkillStore,
) -> io::Result<()> {
    let skills = discover_skills(&skill_roots_for(cwd))?;
    let enabled = store.enabled();
    let context = build_skill_context(&skills, &enabled)?;
    app.set_skills(skills, enabled);
    app.set_skill_context(context);
    Ok(())
}

pub(crate) fn hydrate_thread_from_events(
    thread: StoredThread,
    session_event_store: &SessionEventStore,
) -> io::Result<StoredThread> {
    let events = session_event_store.events(&thread.id)?;
    if events.is_empty() {
        Ok(thread)
    } else {
        Ok(apply_events_to_thread(&thread, &events))
    }
}

pub(crate) fn confirm_project_workspace(
    stdout: &mut io::Stdout,
    workspace: &ProjectWorkspace,
) -> io::Result<bool> {
    let cwd = workspace.cwd().display().to_string();
    let workspace_path = format_path_for_humans(&workspace.project_dir());

    loop {
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        let frame = normalize_for_raw_terminal(&render_project_confirmation(
            width,
            &cwd,
            &workspace_path,
            true,
        ));
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        write!(stdout, "{frame}")?;
        stdout.flush()?;

        if let Event::Key(key) = read()? {
            match key.code {
                KeyCode::Enter => {
                    workspace.confirm()?;
                    return Ok(true);
                }
                KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false);
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn run_boot_sequence(
    stdout: &mut io::Stdout,
    app: &mut App,
    bridge: &mut AcpBridge,
    pending_permission_reply: &mut Option<tokio::sync::oneshot::Sender<Option<String>>>,
) -> io::Result<()> {
    let mut tick = 0usize;
    let minimum_ticks = boot_minimum_ticks();

    loop {
        drain_bridge_events(app, bridge, pending_permission_reply);

        let model = app.selected_model_id().map(str::to_string);
        let ready = model.is_some();
        let detail = model
            .map(|model| format!("ready on {model}"))
            .unwrap_or_else(|| "loading model inventory".to_string());
        let status = if ready { "ready" } else { "loading" };
        let active_step = (!ready).then_some("copilot-session");
        let steps = [BootStep::new("copilot-session", "copilot", status, &detail)];
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        let frame =
            normalize_for_raw_terminal(&render_boot_frame(width, tick, active_step, &steps, true));
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        write!(stdout, "{frame}")?;
        stdout.flush()?;

        if ready && tick >= minimum_ticks {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(80));
        tick = tick.saturating_add(1);
    }

    Ok(())
}

pub(crate) fn drain_bridge_events(
    app: &mut App,
    bridge: &mut AcpBridge,
    pending_permission_reply: &mut Option<tokio::sync::oneshot::Sender<Option<String>>>,
) {
    while let Ok(event) = bridge.evt_rx.try_recv() {
        match event {
            BridgeEvent::TextChunk { text } => app.apply_assistant_chunk(&text),
            BridgeEvent::ToolCall { title } => app.apply_tool_notice(title, None),
            BridgeEvent::ToolUpdate { title, detail } => {
                if let Some(update) = tool_update_text(title, detail) {
                    app.apply_tool_update(update);
                }
            }
            BridgeEvent::PermissionRequest {
                title,
                options,
                reply,
            } => {
                if app.approval_mode() == ApprovalMode::Auto {
                    if let Some(option) = choose_auto_permission(&options) {
                        let _ = reply.send(Some(option.option_id.clone()));
                        app.apply_system_notice(format!("Auto-approved: {}", option.name));
                    } else {
                        let _ = reply.send(None);
                        app.apply_system_notice(format!("Permission cancelled: {title}"));
                    }
                    continue;
                }
                if let Some(previous) = pending_permission_reply.take() {
                    let _ = previous.send(None);
                }
                *pending_permission_reply = Some(reply);
                app.open_permission_prompt(
                    title,
                    options
                        .into_iter()
                        .map(|option| PermissionOptionView {
                            option_id: option.option_id,
                            name: option.name,
                        })
                        .collect(),
                );
            }
            BridgeEvent::SessionReady {
                current_model,
                available_models,
            } => {
                if let Some(current_model) = current_model {
                    app.apply_session_ready(current_model, available_models);
                } else if !available_models.is_empty() {
                    app.set_model_choices(available_models);
                }
            }
            BridgeEvent::ModelChanged { model } => app.apply_model_changed(model),
            BridgeEvent::PromptDone => app.finish_prompt(),
            BridgeEvent::Error { message } => {
                app.apply_system_notice(format!("Error: {message}"));
                app.finish_prompt();
            }
        }
    }
}
