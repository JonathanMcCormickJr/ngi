#[cfg(test)]
mod tests {
    use crate::{
        build_network_factory_and_peers, build_raft_node, initialize_cluster_if_leader,
        make_raft_config, parse_listen_addr, parse_peer_configs, should_initialize_cluster,
    };

    // ── parse_peer_configs ────────────────────────────────────────────────────

    #[test]
    fn parse_peer_configs_handles_valid_and_invalid_entries() {
        let peers = parse_peer_configs(
            "1:http://127.0.0.1:50051,invalid,2:http://127.0.0.1:50052,0:http://x",
        );

        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].node_id, 1);
        assert_eq!(peers[0].address, "http://127.0.0.1:50051");
        assert_eq!(peers[1].node_id, 2);
        assert_eq!(peers[1].address, "http://127.0.0.1:50052");
    }

    #[test]
    fn parse_peer_configs_empty_string_returns_empty() {
        assert!(parse_peer_configs("").is_empty());
    }

    #[test]
    fn parse_peer_configs_single_entry_no_comma() {
        let peers = parse_peer_configs("3:http://10.0.0.3:8080");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].node_id, 3);
        assert_eq!(peers[0].address, "http://10.0.0.3:8080");
    }

    #[test]
    fn parse_peer_configs_preserves_ipv6_colons() {
        // Parts after the first colon are joined with ':' so IPv6 literals survive.
        let peers = parse_peer_configs("1:[::1]:50051");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].node_id, 1);
        assert_eq!(peers[0].address, "[::1]:50051");
    }

    #[test]
    fn parse_peer_configs_trims_whitespace_around_entries() {
        let peers = parse_peer_configs("  1:http://127.0.0.1:50051  ");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].node_id, 1);
    }

    // ── should_initialize_cluster ─────────────────────────────────────────────

    #[test]
    fn should_initialize_cluster_only_for_first_member() {
        assert!(should_initialize_cluster(1, &[1, 2, 3]));
        assert!(!should_initialize_cluster(2, &[1, 2, 3]));
        assert!(!should_initialize_cluster(1, &[]));
    }

    #[test]
    fn should_initialize_cluster_true_when_node_is_sole_peer() {
        // A single-node cluster initialised by that node.
        assert!(should_initialize_cluster(5, &[5]));
    }

    #[test]
    fn should_initialize_cluster_false_when_node_is_not_first() {
        assert!(!should_initialize_cluster(2, &[1]));
    }

    // ── parse_listen_addr ─────────────────────────────────────────────────────

    #[test]
    fn parse_listen_addr_accepts_valid_ipv4() {
        let addr = parse_listen_addr("127.0.0.1:8080").expect("valid IPv4 should parse");
        assert_eq!(addr.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn parse_listen_addr_accepts_valid_ipv6() {
        let addr = parse_listen_addr("[::1]:50051").expect("valid IPv6 should parse");
        assert_eq!(addr.port(), 50051);
    }

    #[test]
    fn parse_listen_addr_rejects_invalid_string() {
        assert!(parse_listen_addr("not-an-addr").is_err());
    }

    // ── make_raft_config ──────────────────────────────────────────────────────

    #[test]
    fn make_raft_config_returns_valid_config() {
        let config = make_raft_config().expect("config creation should succeed");
        assert_eq!(config.heartbeat_interval, 500);
        assert_eq!(config.election_timeout_min, 1500);
        assert_eq!(config.election_timeout_max, 3000);
    }

    // ── build_network_factory_and_peers ───────────────────────────────────────

    #[test]
    fn build_network_factory_and_peers_populates_all_entries() {
        let (_, peers) =
            build_network_factory_and_peers("1:http://127.0.0.1:50051,2:http://127.0.0.1:50052");
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].node_id, 1);
        assert_eq!(peers[0].address, "http://127.0.0.1:50051");
        assert_eq!(peers[1].node_id, 2);
        assert_eq!(peers[1].address, "http://127.0.0.1:50052");
    }

    #[test]
    fn build_network_factory_and_peers_empty_string_returns_empty() {
        let (_, peers) = build_network_factory_and_peers("");
        assert!(peers.is_empty());
    }

    #[test]
    fn build_network_factory_and_peers_skips_invalid_entries() {
        let (_, peers) = build_network_factory_and_peers("1:http://ok:8080,bad,0:http://zero");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].node_id, 1);
    }

    // ── build_raft_node ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn build_raft_node_creates_store_and_raft_instance() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().to_str().expect("utf-8 path").to_string();
        let config = make_raft_config().expect("config");
        let (factory, _) = build_network_factory_and_peers("1:http://127.0.0.1:50080");
        let result = build_raft_node(1, config, factory, &path).await;
        assert!(result.is_ok());
    }

    // ── initialize_cluster_if_leader ───────────────────────────────────────────────

    #[tokio::test]
    async fn initialize_cluster_noop_when_not_first_peer() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().to_str().expect("utf-8 path").to_string();
        let config = make_raft_config().expect("config");
        let (factory, _) = build_network_factory_and_peers("1:http://127.0.0.1:50081");
        let (raft, _) = build_raft_node(1, config, factory, &path)
            .await
            .expect("raft node");
        // node_id=2, first peer is 1 → not the leader → should be a no-op
        initialize_cluster_if_leader(&raft, 2, &[1, 2, 3]).await;
    }

    #[tokio::test]
    async fn initialize_cluster_succeeds_for_sole_member() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().to_str().expect("utf-8 path").to_string();
        let config = make_raft_config().expect("config");
        let (factory, _) = build_network_factory_and_peers("1:http://127.0.0.1:50082");
        let (raft, _) = build_raft_node(1, config, factory, &path)
            .await
            .expect("raft node");
        // node_id=1 is first in [1] → should attempt (and succeed) to bootstrap
        initialize_cluster_if_leader(&raft, 1, &[1]).await;
    }
}
