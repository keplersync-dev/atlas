-- Files the owner shares "first install only": members get them once and keep their own
-- after. JSON list of paths. sync_include and sync_exclude now hold the share tree's ticks.
ALTER TABLE instance ADD COLUMN sync_first_install TEXT NOT NULL DEFAULT '[]';
