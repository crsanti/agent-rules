use serde_json::{Map, Value};

/// Walks a dotted path, creating (or overwriting with) an empty object for
/// any *intermediate* segment that is missing or not itself an object. The
/// final segment is left untouched here -- the caller reads/validates it as
/// an array.
fn navigate_to_list_parent<'a>(root: &'a mut Map<String, Value>, dotted: &str) -> &'a mut Map<String, Value> {
    let keys: Vec<&str> = dotted.split('.').collect();
    let mut node = root;
    for k in &keys[..keys.len().saturating_sub(1)] {
        let entry = node
            .entry((*k).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        node = entry.as_object_mut().unwrap(); // safe: just forced to Object above
    }
    node
}

/// Idempotent remove/ensure operation on the JSON array at `json_path`,
/// leaving every other key untouched. serde_json's `Map`, built with the
/// `preserve_order` feature, keeps insertion order -- so re-serializing an
/// untouched key set reproduces the original file's key order exactly (see
/// README).
pub fn apply_json(target: &str, body: &str, json_path: &str, dry_run: bool) -> Result<crate::Res, String> {
    let spec: Value = if body.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(body).map_err(|e| format!("{target}: invalid JSON spec: {e}"))?
    };
    if !spec.is_object() {
        return Err(format!("{target}: JSON spec is not an object"));
    }
    let remove: Vec<Value> = spec
        .get("remove")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let ensure: Vec<Value> = spec
        .get("ensure")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let (existed, original) = crate::io_util::read_if_exists(target)?;

    let mut data: Value = if original.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(&original).map_err(|e| format!("{target}: invalid JSON: {e}"))?
    };
    if !data.is_object() {
        return Err(format!("{target}: top-level JSON is not an object"));
    }

    let last_key = json_path.rsplit('.').next().unwrap_or(json_path).to_string();

    {
        let root_map = data.as_object_mut().unwrap(); // safe: checked is_object above
        let parent = navigate_to_list_parent(root_map, json_path);
        let current: Vec<Value> = match parent.get(&last_key) {
            None => Vec::new(),
            Some(Value::Array(a)) => a.clone(),
            Some(_) => return Err(format!("{target}: {json_path} is not a JSON array")),
        };
        let mut new_list: Vec<Value> = current
            .into_iter()
            .filter(|x| !remove.iter().any(|r| r == x))
            .collect();
        for item in &ensure {
            if !new_list.iter().any(|x| x == item) {
                new_list.push(item.clone());
            }
        }
        parent.insert(last_key.clone(), Value::Array(new_list));
    }

    let new_text = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())? + "\n";

    if existed && new_text == original {
        return Ok(crate::Res {
            target: target.to_string(),
            marker: json_path.to_string(),
            action: "unchanged".to_string(),
        });
    }
    if !dry_run {
        if existed {
            crate::io_util::backup_file(target)?;
        }
        crate::io_util::atomic_write(target, &new_text)?;
    }
    let mut action = "patched".to_string();
    if dry_run {
        action.push_str(" (dry-run)");
    }
    Ok(crate::Res {
        target: target.to_string(),
        marker: json_path.to_string(),
        action,
    })
}
