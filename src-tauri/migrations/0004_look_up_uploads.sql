-- Uploaded files are now looked up too: one may be on Modrinth or CurseForge, so friends
-- can download it from there instead. Ask about them on the next scan.
UPDATE instance_content SET resolved_at = NULL WHERE source = 'upload';
