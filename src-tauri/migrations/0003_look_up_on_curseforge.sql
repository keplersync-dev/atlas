-- Unknown files are now also looked up on CurseForge. Ask about them again on the next
-- scan instead of waiting out the week since their last (Modrinth-only) lookup.
UPDATE instance_content SET resolved_at = NULL WHERE source = 'local';
