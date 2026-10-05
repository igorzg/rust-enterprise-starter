-- Groups and their many-to-many membership with users.
-- A user can belong to several groups; a group has several members.
CREATE TABLE groups (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT groups_name_unique UNIQUE (name)
);

-- Reuse the set_updated_at() trigger function created in 0001.
CREATE TRIGGER groups_set_updated_at
BEFORE UPDATE ON groups
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Join table: the (user, group) pairs. The composite PK makes each
-- assignment unique; ON DELETE CASCADE keeps it consistent when a user or
-- a group is removed.
CREATE TABLE user_groups (
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    group_id   UUID NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, group_id)
);

-- The composite PK already indexes by user_id (for "groups for a user");
-- add the reverse index for "members of a group".
CREATE INDEX user_groups_group_id_idx ON user_groups (group_id);
