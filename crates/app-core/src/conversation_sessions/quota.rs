use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunReservation {
    pub reservation_id: String,
    pub session_id: String,
    #[serde(deserialize_with = "required_nullable")]
    pub turn_id: Option<String>,
}

fn size(path: &Path) -> Result<u64, AppError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !is_link_or_reparse(&meta) => Ok(meta.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        _ => Err(error("sessions.boundary")),
    }
}
fn used(handle: &ProjectHandle, kind: StoreKind) -> Result<(u64, usize), AppError> {
    let root = handle.root().join(".novelgainker").join(kind.directory());
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, 0)),
        Ok(meta) if meta.is_dir() && !is_link_or_reparse(&meta) => {}
        _ => return Err(error("sessions.boundary")),
    }
    let mut directories = vec![root.clone()];
    let mut count = 0;
    let mut bytes = 0u64;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|_| error("sessions.unavailable"))? {
            let entry = entry.map_err(|_| error("sessions.unavailable"))?;
            count += 1;
            if count > 1024 {
                return Err(error("sessions.full"));
            }
            let meta =
                fs::symlink_metadata(entry.path()).map_err(|_| error("sessions.unavailable"))?;
            if is_link_or_reparse(&meta) {
                return Err(error("sessions.boundary"));
            }
            if meta.is_dir() {
                if directory != root || entry.file_name() != "sessions" {
                    return Err(error("sessions.boundary"));
                }
                directories.push(entry.path());
            } else if meta.is_file() {
                // Catalog has its own 1 MiB bound. Aborted catalog temporaries
                // still count toward the enclosing category's physical usage.
                if directory == root
                    && matches!(kind, StoreKind::Conversation)
                    && entry.file_name() == "catalog.json"
                {
                    continue;
                }
                bytes = bytes
                    .checked_add(meta.len())
                    .ok_or_else(|| error("sessions.full"))?;
            } else {
                return Err(error("sessions.boundary"));
            }
        }
    }
    Ok((bytes, count))
}
pub(super) fn check_write_locked(
    handle: &ProjectHandle,
    catalog: &Catalog,
    kind: StoreKind,
    bytes: u64,
) -> Result<(), AppError> {
    if bytes > kind.single_limit() {
        return Err(error("sessions.full"));
    }
    let target = store_path(handle, kind, true)?;
    let (used_bytes, entries) = used(handle, kind)?;
    // Reserve a temporary directory entry as well as a missing target. Do not
    // accept entry 1025 and discover the overflow only after it is durable.
    let missing_target = usize::from(
        !target
            .try_exists()
            .map_err(|_| error("sessions.unavailable"))?,
    );
    if entries + missing_target + 1 > 1024 {
        return Err(error("sessions.full"));
    }
    let mut charged = used_bytes
        .saturating_sub(size(&target)?)
        .saturating_add(bytes);
    if let Some(run) = &catalog.run {
        let run_handle = binding(handle, catalog, &run.session_id)?;
        let run_path = store_path(&run_handle, kind, true)?;
        let occupied = if run_path == target {
            bytes
        } else {
            size(&run_path)?
        };
        charged = charged.saturating_add(kind.single_limit().saturating_sub(occupied));
    }
    if charged > kind.total_limit() {
        return Err(error("sessions.full"));
    }
    Ok(())
}
pub(crate) fn with_store_write<T>(
    handle: &ProjectHandle,
    kind: StoreKind,
    bytes: u64,
    operation: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, AppError> {
    // Historical NR-5 fixtures and an unregistered legacy handle keep their
    // original store contract. Production NR-6 always captures a bound handle.
    if handle.session_binding().is_none() {
        return operation();
    }
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let catalog = read_catalog(handle)?;
    let selected = handle.session_binding().unwrap();
    if !catalog
        .entries
        .iter()
        .any(|entry| entry.session_id == selected.id)
    {
        return Err(error("sessions.not_found"));
    }
    check_write_locked(handle, &catalog, kind, bytes)?;
    operation()
}
pub fn reserve_run(handle: &ProjectHandle, now: u64) -> Result<RunReservation, AppError> {
    let selected = handle
        .session_binding()
        .ok_or_else(|| error("sessions.invalid"))?;
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut catalog = ensure_catalog(handle, now)?;
    if catalog.run.is_some() {
        return Err(error("sessions.run_active"));
    }
    if !catalog
        .entries
        .iter()
        .any(|entry| entry.session_id == selected.id && !entry.archived)
    {
        return Err(error("sessions.not_found"));
    }
    let reservation = RunReservation {
        reservation_id: Uuid::new_v4().to_string(),
        session_id: selected.id.clone(),
        turn_id: None,
    };
    catalog.run = Some(reservation.clone());
    for kind in [StoreKind::Conversation, StoreKind::Memory, StoreKind::Task] {
        let path = store_path(handle, kind, true)?;
        check_write_locked(handle, &catalog, kind, size(&path)?)?;
    }
    if catalog.revision >= MAX_REVISION {
        return Err(error("sessions.revision_conflict"));
    }
    catalog.revision += 1;
    write_catalog(handle, &catalog)?;
    Ok(reservation)
}
pub fn bind_run_turn(
    handle: &ProjectHandle,
    reservation_id: &str,
    turn_id: &str,
) -> Result<(), AppError> {
    if !valid_id(turn_id) {
        return Err(error("sessions.invalid"));
    }
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut catalog = read_catalog(handle)?;
    let run = catalog
        .run
        .as_mut()
        .ok_or_else(|| error("sessions.run_missing"))?;
    if run.reservation_id != reservation_id || run.turn_id.as_ref().is_some_and(|id| id != turn_id)
    {
        return Err(error("sessions.revision_conflict"));
    }
    if run.turn_id.as_deref() == Some(turn_id) {
        return Ok(());
    }
    run.turn_id = Some(turn_id.into());
    if catalog.revision >= MAX_REVISION {
        return Err(error("sessions.revision_conflict"));
    }
    catalog.revision += 1;
    write_catalog(handle, &catalog)
}
pub fn release_run(handle: &ProjectHandle, reservation_id: &str) -> Result<(), AppError> {
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut catalog = read_catalog(handle)?;
    let Some(run) = &catalog.run else {
        return Ok(());
    };
    if run.reservation_id != reservation_id {
        return Err(error("sessions.revision_conflict"));
    }
    if catalog.revision >= MAX_REVISION {
        return Err(error("sessions.revision_conflict"));
    }
    catalog.run = None;
    catalog.revision += 1;
    write_catalog(handle, &catalog)
}
