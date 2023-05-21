pub trait SDK {
    fn submit_delivery_acknowledgment();

    fn submit_reputation_score();

    fn submit_transaction();

    fn fs_exists();

    fn fs_put();

    fn fs_get_tree();

    fn fs_get_chunk();

    fn fs_request_download();

    fn get_epoch_randomness();

    fn gossip_on_message();

    fn gossip_send_message();

    fn query_balance();

    fn query_nodes();

    fn ev_on_new_connection();

    fn ev_on_epoch_change();

    fn ev_on_detach();
}
