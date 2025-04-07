BEGIN;
	CREATE TABLE IF NOT EXISTS request  (
	request_id INTEGER PRIMARY KEY AUTOINCREMENT,
	timestamp INTEGER NOT NULL
) STRICT;

-- Not needed
CREATE TABLE IF NOT EXISTS timezone  (
	timezone_id INTEGER PRIMARY KEY AUTOINCREMENT CHECK (timezone_id = 1),
	zone_name TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS skip_request  (
	skip_request_id INTEGER PRIMARY KEY AUTOINCREMENT CHECK (skip_request_id = 1),
	skip INTEGER NOT NULL CHECK (skip IN (0, 1))
) STRICT;

COMMIT;