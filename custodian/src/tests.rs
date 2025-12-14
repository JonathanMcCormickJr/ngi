#[cfg(test)]
mod tests {
    use custodian::storage::Storage;
    use custodian::LockCommand;
    use uuid::Uuid;

    #[test]
    fn test_lock_operations() {
        let storage = Storage::new_temp().unwrap();
        let ticket_id = 42;
        let user_id = Uuid::new_v4();

        // Initially not locked
        assert!(!storage.is_locked(ticket_id).unwrap());

        // Acquire lock
        storage.acquire_lock(ticket_id, user_id).unwrap();
        assert!(storage.is_locked(ticket_id).unwrap());

        // Get lock info
        let lock_info = storage.get_lock_info(ticket_id).unwrap().unwrap();
        assert_eq!(lock_info.ticket_id, ticket_id);
        assert_eq!(lock_info.user_id, user_id);

        // Release lock
        storage.release_lock(ticket_id).unwrap();
        assert!(!storage.is_locked(ticket_id).unwrap());
    }

    #[test]
    fn test_get_all_locks() {
        let storage = Storage::new_temp().unwrap();
        let user_id = Uuid::new_v4();

        // Add multiple locks
        storage.acquire_lock(1, user_id).unwrap();
        storage.acquire_lock(2, user_id).unwrap();

        let locks = storage.get_all_locks().unwrap();
        assert_eq!(locks.len(), 2);
        assert!(locks.contains_key(&1));
        assert!(locks.contains_key(&2));
    }

    #[test]
    fn test_lock_command_apply() {
        let storage = Storage::new_temp().unwrap();
        let ticket_id = 123;
        let user_id = Uuid::new_v4();

        // Test acquire lock command
        let acquire_cmd = LockCommand::AcquireLock { ticket_id, user_id };
        acquire_cmd.apply(&storage).unwrap();
        assert!(storage.is_locked(ticket_id).unwrap());

        // Test release lock command
        let release_cmd = LockCommand::ReleaseLock { ticket_id, user_id };
        release_cmd.apply(&storage).unwrap();
        assert!(!storage.is_locked(ticket_id).unwrap());
    }
}
