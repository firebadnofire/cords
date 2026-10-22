-- Atomic hierarchy edits share the existing role revision, audit and receipt.
-- No activation of staged account roles occurs in this additive migration.
ALTER TABLE role_policy_changes DROP CONSTRAINT role_policy_changes_operation_check;
ALTER TABLE role_policy_changes ADD CONSTRAINT role_policy_changes_operation_check
    CHECK(operation IN ('initialize','put_role','delete_role','assign_role','revoke_role','reorder_roles'));
UPDATE cords_schema_metadata SET version=21 WHERE singleton=TRUE;
