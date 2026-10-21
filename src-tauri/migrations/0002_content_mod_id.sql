-- The id a jar declares about itself (fabric.mod.json, mods.toml, mcmod.info, ...).
-- Lets Atlas recognise a mod across updates (sodium-0.5.jar -> sodium-0.6.jar is still
-- "sodium"), so the user's share setting carries over. NULL for packs and unreadable jars.
ALTER TABLE instance_content ADD COLUMN mod_id TEXT;
