use std::cell::RefCell;
use std::rc::Rc;

use ego_tree::NodeRef as EgoNodeRef;
use scraper::{Html, Node as ScraperNode};

pub type NodeRef = Rc<RefCell<Node>>;

pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<NodeRef>,
}

pub enum NodeKind {
    Document,
    Element {
        tag: String,
        attrs: Vec<(String, String)>,
    },
    Text(String),
    Comment(String),
    Doctype,
}

pub fn parse(html: &str) -> Node {
    let document = Html::parse_document(html);
    convert(document.tree.root())
}

fn convert(node_ref: ego_tree::NodeRef<scraper::Node>) -> Node {
    let kind = match node_ref.value() {
        ScraperNode::Document | ScraperNode::Fragment => NodeKind::Document,
        ScraperNode::Doctype(_) | ScraperNode::ProcessingInstruction(_) => NodeKind::Doctype,
        ScraperNode::Element(el) => NodeKind::Element {
            tag: el.name().to_string(),
            attrs: el.attrs().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        },
        ScraperNode::Text(text) => NodeKind::Text(text.to_string()),
        ScraperNode::Comment(text) => NodeKind::Comment(text.to_string()),
    };

    let children = node_ref
        .children()
        .filter(|c| !is_ignored(c.value()))
        .map(|c| Rc::new(RefCell::new(convert(c))))
        .collect();

    Node { kind, children }
}

fn is_ignored(node: &ScraperNode) -> bool {
    matches!(node, ScraperNode::Comment(_) | ScraperNode::Doctype(_) | ScraperNode::ProcessingInstruction(_))
}
