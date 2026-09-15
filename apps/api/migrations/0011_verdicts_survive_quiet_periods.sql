-- A verdict now survives a quiet period instead of decaying into UNKNOWN.
--
-- The old hour was written for a service with continuous traffic. An
-- integration called a few times a day spent most of its life labelled
-- UNKNOWN, which read identically to one nobody had ever instrumented, and
-- that is the opposite of useful: "it worked an hour ago and has been quiet
-- since" is not the same fact as "we have never seen this run".
--
-- Twelve hours instead. Past that the last verdict really is too old to stand
-- behind, and UNKNOWN becomes honest again.
ALTER TABLE health_policies ALTER COLUMN stale_after_minutes SET DEFAULT 720;

-- Move policies still sitting on the old default. A policy deliberately set to
-- something else is left alone; one that still reads 60 was never chosen, it
-- was inherited.
UPDATE health_policies SET stale_after_minutes = 720 WHERE stale_after_minutes = 60;
