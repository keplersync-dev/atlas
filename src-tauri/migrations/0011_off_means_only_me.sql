-- Turning a file off now only changes your own game: files already off become Only me, so friends get what they got.
UPDATE instance_content SET share_mode = 'private'
WHERE installed = 1 AND enabled = 0
  AND instance_id IN (SELECT id FROM instance WHERE shared_role IS NULL OR shared_role != 'viewer');
