//! `servers.dat`, the multiplayer server list: an owner's servers join each member's
//! list, which keeps the member's own, as the Modrinth App does with modpack servers.

use std::collections::HashSet;

use fastnbt::Value;

/// The member's list with the owner's servers first: an owner's server replaces the
/// member's entry for the same address, and every other entry stays. None when either
/// file can't be read, so the member's file is left as it is.
///
/// shortcut: a server the owner later removes stays in members' lists; tracking the
/// owner's previous list would fix it, if people ask.
pub fn merge(mine: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    let mut mine: Value = fastnbt::from_bytes(mine).ok()?;
    let theirs: Value = fastnbt::from_bytes(theirs).ok()?;
    let theirs = servers(&theirs)?.clone();
    let addresses: HashSet<String> = theirs.iter().filter_map(address).collect();
    let Value::Compound(root) = &mut mine else {
        return None;
    };
    let own = match root.get("servers") {
        Some(Value::List(list)) => list.clone(),
        None => Vec::new(),
        Some(_) => return None,
    };
    let mut merged = theirs;
    merged.extend(
        own.into_iter()
            .filter(|s| address(s).is_none_or(|a| !addresses.contains(&a))),
    );
    root.insert("servers".to_owned(), Value::List(merged));
    fastnbt::to_bytes(&mine).ok()
}

fn servers(v: &Value) -> Option<&Vec<Value>> {
    match v {
        Value::Compound(root) => match root.get("servers") {
            Some(Value::List(list)) => Some(list),
            _ => None,
        },
        _ => None,
    }
}

/// Compared without case or a default port, as players type them either way.
fn address(server: &Value) -> Option<String> {
    let Value::Compound(s) = server else {
        return None;
    };
    let Some(Value::String(ip)) = s.get("ip") else {
        return None;
    };
    let ip = ip.trim().to_ascii_lowercase();
    Some(ip.strip_suffix(":25565").unwrap_or(&ip).to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn list(servers: &[(&str, &str)]) -> Vec<u8> {
        let entries = servers
            .iter()
            .map(|(name, ip)| {
                Value::Compound(HashMap::from([
                    ("name".to_owned(), Value::String(name.to_string())),
                    ("ip".to_owned(), Value::String(ip.to_string())),
                ]))
            })
            .collect();
        let root = Value::Compound(HashMap::from([(
            "servers".to_owned(),
            Value::List(entries),
        )]));
        fastnbt::to_bytes(&root).unwrap()
    }

    fn names(bytes: &[u8]) -> Vec<String> {
        let v: Value = fastnbt::from_bytes(bytes).unwrap();
        servers(&v)
            .unwrap()
            .iter()
            .map(|s| match s {
                Value::Compound(c) => match c.get("name") {
                    Some(Value::String(n)) => n.clone(),
                    _ => panic!("no name"),
                },
                _ => panic!("not a server"),
            })
            .collect()
    }

    #[test]
    fn the_owners_servers_join_the_members_own() {
        let mine = list(&[
            ("My SMP", "play.mine.net"),
            ("Old Create", "Create.Example.com:25565"),
        ]);
        let theirs = list(&[("Create SMP", "create.example.com")]);
        let merged = merge(&mine, &theirs).unwrap();
        assert_eq!(names(&merged), ["Create SMP", "My SMP"]);
    }

    #[test]
    fn a_file_that_cant_be_read_is_left_alone() {
        let theirs = list(&[("Create SMP", "create.example.com")]);
        assert_eq!(merge(b"not nbt", &theirs), None);
        assert_eq!(merge(&theirs, b"not nbt"), None);
    }
}
