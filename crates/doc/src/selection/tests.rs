use super::*;

#[test]
fn basic_ops() {
    let mut s = Selection::default();
    s.add(NodeId(1));
    s.add(NodeId(1));
    s.toggle(NodeId(2));
    assert_eq!(s.objects, vec![NodeId(1), NodeId(2)]);
    s.toggle(NodeId(1));
    assert_eq!(s.objects, vec![NodeId(2)]);
    s.clear();
    assert!(s.is_empty());
}

#[test]
fn guides_are_selected_on_their_own() {
    let mut s = Selection::default();
    s.add(NodeId(1));
    s.set_guides([2, 0, 2]);
    assert_eq!((s.objects.clone(), s.guides.clone()), (vec![], vec![2, 0]));
    assert!(s.has_objects_or_guides() && s.is_empty());
    // Shift-adding an object keeps them; a new selection doesn't.
    s.add(NodeId(1));
    assert_eq!(s.guides, vec![2, 0]);
    s.set([NodeId(1)]);
    assert!(s.guides.is_empty());
    // Toggled, a guide joins (or leaves) the selected objects.
    s.toggle_guide(0);
    assert_eq!((s.objects.clone(), s.guides.clone()), (vec![NodeId(1)], vec![0]));
    s.toggle_guide(0);
    assert_eq!((s.objects.clone(), s.guides.clone()), (vec![NodeId(1)], vec![]));
    // Pruning drops guides the document no longer has.
    let mut d = Document::new(100.0, 100.0);
    d.guides.push(crate::Guide::new(true, 10.0));
    s.set_guides([0, 1]);
    s.prune(&d);
    assert_eq!(s.guides, vec![0]);
}

#[test]
fn guides_show_and_lock_with_their_layer() {
    let mut d = Document::new(100.0, 100.0);
    let layer = d.layers[0].id;
    let g = crate::Guide { layer: Some(layer), ..crate::Guide::new(true, 10.0) };
    assert!(d.guide_shown(&g) && d.guide_editable(&g));
    d.node_mut(layer).unwrap().locked = true;
    assert!(d.guide_shown(&g) && !d.guide_editable(&g), "locked: shown, not editable");
    d.node_mut(layer).unwrap().visible = false;
    assert!(!d.guide_shown(&g) && !d.guide_editable(&g), "hidden");
    // On no layer, or a layer that's gone: always there.
    assert!(d.guide_shown(&crate::Guide::new(false, 5.0)));
    let gone = crate::Guide { layer: Some(NodeId(9999)), ..crate::Guide::new(true, 1.0) };
    assert!(d.guide_shown(&gone) && d.guide_editable(&gone));
    d.guides.push(g);
    assert_eq!(d.guides_on(layer).count(), 1);
    // Guides on no layer, or a gone one, are adopted by the top visible, unlocked layer.
    d.node_mut(layer).unwrap().visible = true;
    d.node_mut(layer).unwrap().locked = false;
    d.guides.push(crate::Guide::new(false, 5.0));
    d.guides.push(gone);
    assert_eq!(d.adopt_guides(), 2);
    assert!(d.guides.iter().all(|g| g.layer == Some(layer)));
    assert_eq!(d.adopt_guides(), 0, "once");
    // A guide saved before guides had layers reads as one on no layer.
    let old: crate::Guide = serde_json::from_str(r#"{"vertical": true, "pos": 5.0}"#).unwrap();
    assert_eq!(old.layer, None);
    assert!(!serde_json::to_string(&crate::Guide::new(true, 5.0)).unwrap().contains("layer"), "nothing new written");
}

#[test]
fn selection_changes_clear_the_target() {
    let mut s = Selection::default();
    s.set([NodeId(2), NodeId(3)]);
    s.target = Some(NodeId(1));
    assert_eq!(s.subjects(), &[NodeId(1)]);
    s.add(NodeId(4));
    assert_eq!(s.target, None);
    assert_eq!(s.subjects(), &[NodeId(2), NodeId(3), NodeId(4)]);
    s.target = Some(NodeId(1));
    s.remove(NodeId(2));
    assert_eq!(s.target, None);
    s.target = Some(NodeId(1));
    s.set([NodeId(3)]);
    assert_eq!(s.target, None);
}

#[test]
fn a_key_object_needs_two_or_more_selected_objects() {
    let mut s = Selection::default();
    s.set([NodeId(1), NodeId(2)]);
    s.key = Some(NodeId(2));
    s.remove(NodeId(1));
    assert_eq!(s.key, Some(NodeId(2)), "another object left: remove keeps the key");
    s.remove(NodeId(2));
    assert_eq!(s.key, None, "the key object itself left");
    // Pruning drops objects the document no longer has and clears the key once no other
    // selected object lies outside it.
    let mut d = Document::new(100.0, 100.0);
    let layer = d.layers[0].id;
    let mut ids = vec![];
    for _ in 0..4 {
        let id = d.alloc_id();
        ids.push(d.insert(Some(layer), usize::MAX, crate::Node::group(id, vec![])).unwrap());
    }
    s.set(ids.iter().copied());
    s.key = Some(ids[3]);
    d.remove(ids[0]).unwrap();
    s.prune(&d);
    assert_eq!(s.key, Some(ids[3]), "three objects left: the key stays");
    s.toggle(ids[1]);
    s.prune(&d);
    assert_eq!(s.key, Some(ids[3]), "two objects left: the key stays");
    s.toggle(ids[2]);
    s.prune(&d);
    assert_eq!((s.objects.clone(), s.key), (vec![ids[3]], None), "one object left");
    // A group key selected with only its own member is cleared too; another selected object
    // outside the group keeps it.
    let member = d.alloc_id();
    d.insert(Some(ids[3]), usize::MAX, crate::Node::group(member, vec![])).unwrap();
    s.set([member, ids[3]]);
    s.key = Some(ids[3]);
    s.prune(&d);
    assert_eq!(s.key, None, "a group and its own member");
    s.set([member, ids[3], ids[2]]);
    s.key = Some(ids[3]);
    s.prune(&d);
    assert_eq!(s.key, Some(ids[3]), "another object outside the group");
}
