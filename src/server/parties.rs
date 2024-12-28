use std::collections::{HashMap, HashSet};

use crate::player::{steamid_64_to_32, Player, Steamid32};

// taken from https://sashamaps.net/docs/resources/20-colors/
const COLOR_PALETTE: [egui::Color32; 21] = [
    egui::Color32::from_rgb(230, 25, 75),
    egui::Color32::from_rgb(60, 180, 75),
    egui::Color32::from_rgb(255, 225, 25),
    egui::Color32::from_rgb(0, 130, 200),
    egui::Color32::from_rgb(245, 130, 48),
    egui::Color32::from_rgb(145, 30, 180),
    egui::Color32::from_rgb(70, 240, 240),
    egui::Color32::from_rgb(240, 50, 230),
    egui::Color32::from_rgb(210, 245, 60),
    egui::Color32::from_rgb(250, 190, 212),
    egui::Color32::from_rgb(0, 128, 128),
    egui::Color32::from_rgb(220, 190, 255),
    egui::Color32::from_rgb(170, 110, 40),
    egui::Color32::from_rgb(255, 250, 200),
    egui::Color32::from_rgb(128, 0, 0),
    egui::Color32::from_rgb(170, 255, 195),
    egui::Color32::from_rgb(128, 128, 0),
    egui::Color32::from_rgb(255, 215, 180),
    egui::Color32::from_rgb(0, 0, 128),
    egui::Color32::from_rgb(128, 128, 128),
    egui::Color32::from_rgb(255, 255, 255),
];

const INDICATOR_SYMBOLS: [char; 4] = ['■', '★', '▲', '◆'];

/// Structure used to determine which players in the current server are friends
pub struct Parties {
    players: Vec<Steamid32>,

    parties: Vec<HashSet<Steamid32>>,

    pub graph: petgraph::Graph<String, (), petgraph::Directed>,
}

impl Parties {
    pub fn new() -> Parties {
        Parties {
            players: Vec::new(),
            parties: Vec::new(),
            graph: petgraph::stable_graph::StableDiGraph::new(),
        }
    }

    pub fn clear(&mut self) {
        self.players.clear();
        self.parties.clear();
        self.graph.clear();
    }

    /// Updates the internal graph of players
    pub fn update(&mut self, player_map: &HashMap<Steamid32, Player>) {
        // Copy over the players
        self.players = player_map.keys().cloned().collect();
        self.graph.clear();

        for p in &self.players {
            self.graph.add_node(p.clone());
        }

        // Get friends of each player and add them to the graph
        for p in player_map.values() {
            if let Some(Ok(acif)) = &p.account_info {
                if let Some(Ok(friends)) = &acif.friends {
                    let node_ind = self
                        .graph
                        .node_indices()
                        .find(|ind| self.graph[*ind] == p.steamid32)
                        .unwrap();
                    for f in friends {
                        let id = steamid_64_to_32(&f.steamid).unwrap();
                        if self.players.contains(&id) {
                            let friend_ind = self
                                .graph
                                .node_indices()
                                .find(|ind| self.graph[*ind] == id)
                                .unwrap();
                            self.graph.update_edge(node_ind, friend_ind, ());
                        }
                    }
                }
            }
        }

        self.find_parties();
    }

    pub fn get_player_party_indicator(&self, p: &Player) -> Option<(char, egui::Color32)> {
        self.parties
            .iter()
            .position(|party| party.contains(&p.steamid32))
            .map(|ind| {
                (
                    INDICATOR_SYMBOLS[ind / COLOR_PALETTE.len()],
                    COLOR_PALETTE[ind % COLOR_PALETTE.len()],
                )
            })
    }

    /// Determines the connected components of the player graph (aka. the friend groups)
    fn find_parties(&mut self) {
        if self.players.is_empty() {
            self.parties.clear();
            return;
        }
        // TODO: replace with UndirectedAdapter when it gets released
        let mut undir_graph = petgraph::Graph::new_undirected();
        let mut node_map: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> =
            HashMap::new();
        for node in self.graph.node_indices() {
            node_map.insert(
                node,
                undir_graph.add_node(self.graph.node_weight(node).unwrap()),
            );
        }
        for edge in self.graph.edge_indices() {
            let (src, target) = self.graph.edge_endpoints(edge).unwrap();
            undir_graph.update_edge(node_map[&src], node_map[&target], ());
        }
        let party_graph = petgraph::algo::condensation(undir_graph, true);
        assert_eq!(party_graph.edge_count(), 0);
        self.parties = party_graph
            .node_weights()
            .map(|party| HashSet::from_iter(party.iter().cloned().cloned()))
            .filter(|party| party.len() > 1)
            .collect();
    }
}
