use std::env;

#[derive(Clone)]
pub struct Config {
    pub db_path: String,
    pub maxmind_db_path: String,
    pub tracker_port: u16,
    pub lgtn_node_address: String,
    pub lgtn_node_port: u16,
    pub pagination_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_path: env::var("DB_PATH")
                .unwrap_or("~/.lightning/data/geo_info_db".to_string())
                .parse()
                .unwrap(),
            maxmind_db_path: env::var("MAXMIND_DB_PATH")
                .unwrap_or("~/.lightning/data/maxminddb_city".to_string()),
            tracker_port: env::var("TRACKER_PORT")
                .unwrap_or("4000".to_string())
                .parse()
                .unwrap_or(4000),
            lgtn_node_address: env::var("LGTN_ADDRESS").expect("LGTN_ADDRESS should be set"),
            lgtn_node_port: env::var("LGTN_PORT")
                .expect("LGTN_PORT should be set")
                .parse()
                .expect("LGTN_PORT should be an integer"),
            pagination_limit: 500,
        }
    }
}
