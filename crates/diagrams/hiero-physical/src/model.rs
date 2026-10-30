//! The physical diagram's kinds, element and arrow properties, and colours.

use hiero_core::count::InstanceCount;
use hiero_core::figures::PhysicalFigure;
use hiero_core::geometry::Size;
use hiero_core::model::{ColorScheme, KindInfo};
use hiero_core::palette;
use serde::{Deserialize, Serialize};

/// The physical diagram type.
pub struct Physical;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhysicalKind {
    User,
    Server,
    Database,
    Firewall,
    LoadBalancer,
    Gateway,
    Storage,
    Network,
    Zone,
}

/// Which database a database is. Unset means a database of no particular kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatabaseType {
    Postgresql,
    Mysql,
    Oracle,
    Mongodb,
    Mssql,
    Cassandra,
}

/// A database's name, the abbreviation shown on its figure, and its usual port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabaseInfo {
    pub label: &'static str,
    pub abbreviation: &'static str,
    pub port: &'static str,
}

/// A database of no particular kind: shown as "DB", with PostgreSQL's port.
pub const GENERIC_DATABASE: DatabaseInfo = DatabaseInfo {
    label: "Database",
    abbreviation: "DB",
    port: "5432",
};

impl DatabaseType {
    pub const ALL: [Self; 6] = [
        Self::Postgresql,
        Self::Mysql,
        Self::Oracle,
        Self::Mongodb,
        Self::Mssql,
        Self::Cassandra,
    ];

    pub const fn info(self) -> DatabaseInfo {
        let (label, abbreviation, port) = match self {
            Self::Postgresql => ("PostgreSQL", "PSQL", "5432"),
            Self::Mysql => ("MySQL", "MySQL", "3306"),
            Self::Oracle => ("Oracle", "ORCL", "1521"),
            Self::Mongodb => ("MongoDB", "Mongo", "27017"),
            Self::Mssql => ("Microsoft SQL Server", "MSSQL", "1433"),
            Self::Cassandra => ("Cassandra", "Cass", "9042"),
        };
        DatabaseInfo {
            label,
            abbreviation,
            port,
        }
    }
}

/// What a server is other than physical, which sets its figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerType {
    VirtualMachine,
    Container,
}

impl ServerType {
    pub const ALL: [Self; 2] = [Self::VirtualMachine, Self::Container];

    pub const fn label(self) -> &'static str {
        match self {
            Self::VirtualMachine => "Virtual Machine",
            Self::Container => "Container",
        }
    }

    pub const fn figure(self) -> PhysicalFigure {
        match self {
            Self::VirtualMachine => PhysicalFigure::VirtualMachine,
            Self::Container => PhysicalFigure::ContainerStack,
        }
    }
}

/// The colour of a zone outside a network other than blue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ZoneColor {
    Red,
    Yellow,
    Green,
}

/// A zone colour's label, fill, and darker border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneColorInfo {
    pub label: &'static str,
    pub fill: &'static str,
    pub border: &'static str,
}

pub const BLUE_ZONE: ZoneColorInfo = ZoneColorInfo {
    label: "Blue",
    fill: "#dae8fc",
    border: "#2d5f9e",
};

impl ZoneColor {
    pub const ALL: [Self; 3] = [Self::Red, Self::Yellow, Self::Green];

    pub const fn info(self) -> ZoneColorInfo {
        match self {
            Self::Red => ZoneColorInfo {
                label: "Red",
                fill: palette::EXTERNAL_DEPENDENCY.fill,
                border: "#9c3530",
            },
            Self::Yellow => ZoneColorInfo {
                label: "Yellow",
                fill: palette::INTERNAL_DEPENDENCY.fill,
                border: "#806614",
            },
            Self::Green => ZoneColorInfo {
                label: "Green",
                fill: palette::CORE_COMPONENT.fill,
                border: "#356b24",
            },
        }
    }
}

/// What a physical element can have beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalProps {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_type: Option<DatabaseType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_type: Option<ServerType>,
    /// How many run, such as a database's nodes, or a range such as "50-100". Unset means one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_count: Option<InstanceCount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_color: Option<ZoneColor>,
    /// A zone behind a firewall: each arrow into it from outside shows one on its edge.
    #[serde(default, skip_serializing_if = "is_false")]
    pub firewall: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

/// What travels along an arrow and the port it's sent to, such as HTTPS on 443.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalArrow {
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub port: String,
}

impl PhysicalArrow {
    /// Such as "HTTPS:443".
    pub fn tag(&self) -> String {
        [self.protocol.as_str(), self.port.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// The longest protocol and port, and the most instances.
pub const PROTOCOL_MAX: usize = 20;
pub const PORT_MAX: usize = 20;
pub const MAX_INSTANCE_COUNT: u32 = 9999;

/// The protocol and port a new arrow starts with, as most traffic is web traffic.
pub const DEFAULT_PROTOCOL: &str = "HTTPS";
pub const DEFAULT_PORT: &str = "443";
/// The protocol a new arrow to a database starts with.
pub const DATABASE_PROTOCOL: &str = "TCP";

/// The colours a physical diagram is drawn in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhysicalColors {
    pub users: String,
    pub servers: String,
    pub data: String,
    pub appliances: String,
    pub networks: String,
    pub zones: String,
    pub arrows: String,
    pub protocols: String,
    pub lines: String,
    pub labels: String,
    pub network_names: String,
    pub zone_names: String,
}

impl Default for PhysicalColors {
    fn default() -> Self {
        Self {
            users: "#d0e6fb".into(),
            servers: "#c5e0b4".into(),
            data: "#ffe599".into(),
            appliances: "#f6c6d7".into(),
            networks: "#e0e0e0".into(),
            zones: "#f5f5f5".into(),
            arrows: "#0066cc".into(),
            protocols: "#eeeeee".into(),
            lines: "#333333".into(),
            labels: "#0b3d91".into(),
            network_names: "#0b3d91".into(),
            zone_names: "#0b3d91".into(),
        }
    }
}

impl ColorScheme for PhysicalColors {
    const FIELDS: &'static [(&'static str, &'static str)] = &[
        ("users", "Users"),
        ("servers", "Servers"),
        ("data", "Databases and Storage"),
        ("appliances", "Firewalls, Load Balancers, and Gateways"),
        ("networks", "Networks"),
        ("zones", "Zones"),
        ("arrows", "Arrows"),
        ("protocols", "Protocols"),
        ("lines", "Lines"),
        ("labels", "Labels"),
        ("networkNames", "Network Names"),
        ("zoneNames", "Zone Names"),
    ];
}

/// A zone's border, and a network's.
pub const BORDER_WIDTH: f64 = 2.0;
/// The size of a new network and of the zones that divide it.
pub const NETWORK_SIZE: Size = Size::new(600.0, 240.0);
pub const ZONE_SIZE: Size = Size::new(600.0, 200.0);
/// The area of every figure: the figure at the top, with room for a two-line name.
pub const FIGURE_AREA: Size = Size::new(120.0, 100.0);
/// The zones a network is divided into when there's no other network to copy.
pub const DEFAULT_NETWORK_ZONES: [&str; 3] = ["Perimeter", "Business", "Restricted"];

use PhysicalKind as K;

/// Kinds of element that arrows join, any of them to any other.
const CONNECTED: &[PhysicalKind] = &[
    K::User,
    K::Server,
    K::Database,
    K::Firewall,
    K::LoadBalancer,
    K::Gateway,
    K::Storage,
];

const fn figure_kind(kind: PhysicalKind, label: &'static str) -> KindInfo<PhysicalKind> {
    KindInfo {
        size: Some(FIGURE_AREA),
        connects_to: CONNECTED,
        ..KindInfo::new(kind, label)
    }
}

/// The elements offered in a physical diagram's palette.
pub static PALETTE: [KindInfo<PhysicalKind>; 9] = [
    figure_kind(K::User, "User"),
    figure_kind(K::Server, "Server"),
    figure_kind(K::Database, "Database"),
    figure_kind(K::Storage, "Storage"),
    figure_kind(K::Firewall, "Firewall"),
    figure_kind(K::LoadBalancer, "Load Balancer"),
    figure_kind(K::Gateway, "Gateway"),
    KindInfo {
        can_contain: &[K::Zone],
        padding: Some(0.0),
        size: Some(NETWORK_SIZE),
        ..KindInfo::new(K::Network, "Network")
    },
    KindInfo {
        can_contain: &[
            K::User,
            K::Server,
            K::Database,
            K::LoadBalancer,
            K::Gateway,
            K::Storage,
        ],
        header: Some(20.0),
        footer: Some(20.0),
        size: Some(ZONE_SIZE),
        ..KindInfo::new(K::Zone, "Zone")
    },
];

/// Which of the diagram's colours fills an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Users,
    Servers,
    Data,
    Appliances,
    Networks,
    Zones,
}

impl Fill {
    pub fn of(self, c: &PhysicalColors) -> &str {
        match self {
            Self::Users => &c.users,
            Self::Servers => &c.servers,
            Self::Data => &c.data,
            Self::Appliances => &c.appliances,
            Self::Networks => &c.networks,
            Self::Zones => &c.zones,
        }
    }
}

impl PhysicalKind {
    pub const fn fill(self) -> Fill {
        match self {
            Self::User => Fill::Users,
            Self::Server => Fill::Servers,
            Self::Database | Self::Storage => Fill::Data,
            Self::Firewall | Self::LoadBalancer | Self::Gateway => Fill::Appliances,
            Self::Network => Fill::Networks,
            Self::Zone => Fill::Zones,
        }
    }

    /// The figure a kind is drawn as. Networks and zones are regions.
    pub const fn figure(self) -> Option<PhysicalFigure> {
        Some(match self {
            Self::User => PhysicalFigure::User,
            Self::Server => PhysicalFigure::Server,
            Self::Database => PhysicalFigure::DatabaseCylinder,
            Self::Firewall => PhysicalFigure::Firewall,
            Self::LoadBalancer => PhysicalFigure::LoadBalancer,
            Self::Gateway => PhysicalFigure::Gateway,
            Self::Storage => PhysicalFigure::StorageArray,
            Self::Network | Self::Zone => return None,
        })
    }

    /// Whether elements of this kind stack top to bottom in their container, as zones divide
    /// a network.
    pub const fn stacks(self) -> bool {
        matches!(self, Self::Zone)
    }

    /// Whether this kind holds stacked elements, as a network holds zones.
    pub const fn holds_stack(self) -> bool {
        matches!(self, Self::Network)
    }

    pub const fn has_zone_color(self) -> bool {
        matches!(self, Self::Zone)
    }

    pub const fn has_firewall(self) -> bool {
        matches!(self, Self::Zone)
    }

    pub const fn has_database_type(self) -> bool {
        matches!(self, Self::Database)
    }

    pub const fn has_server_type(self) -> bool {
        matches!(self, Self::Server)
    }

    /// What the instance count is called, and what each one is.
    pub const fn instances(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Server => Some(("Instance count", "instance")),
            Self::Database => Some(("Node count", "node")),
            _ => None,
        }
    }

    /// The protocol and port a new arrow to this kind starts with, apart from databases.
    pub const fn arrow_defaults(self) -> (&'static str, &'static str) {
        match self {
            // File shares over SMB.
            Self::Storage => ("TCP", "445"),
            _ => (DEFAULT_PROTOCOL, DEFAULT_PORT),
        }
    }
}
