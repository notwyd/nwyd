use crate::widget::{ListState, ScrollBarRects};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemOptions {
    pub payload: i32,
    pub tag: i32,
    pub font: u8,
    pub marker: bool,
    pub color: u32,
    pub background: u32,
    pub shadow_color: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    parent: Option<usize>,
    pub tag: i32,
    pub expanded: bool,
    pub marker: bool,
    pub font: u8,
}

impl Node {
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    nodes: Vec<Node>,
    pub show_markers: bool,
    pub suppress_marker_hover: bool,
    pub hovered: Option<usize>,
    pub(crate) drag_offset: Option<i32>,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            show_markers: true,
            suppress_marker_hover: false,
            hovered: None,
            drag_offset: None,
        }
    }
}

pub struct Layout {
    pub rows: Vec<(usize, [i32; 4])>,
    pub visible: Vec<usize>,
    pub first: usize,
    pub page: usize,
    pub range: [i32; 2],
    pub bar: ScrollBarRects,
}

impl Tree {
    pub(crate) fn push_root(&mut self, count: usize) {
        self.nodes.truncate(count);

        while self.nodes.len() <= count {
            self.nodes.push(Node {
                parent: None,
                tag: 0,
                expanded: false,
                marker: false,
                font: 0,
            });
        }
    }

    pub fn node(&self, index: usize) -> Option<&Node> {
        self.nodes.get(index)
    }

    pub fn node_mut(&mut self, index: usize) -> Option<&mut Node> {
        self.nodes.get_mut(index)
    }

    pub fn has_children(&self, index: usize) -> bool {
        self.nodes.iter().any(|n| n.parent == Some(index))
    }

    pub fn depth(&self, index: usize) -> usize {
        let mut depth = 1;

        let mut parent = self.node(index).and_then(Node::parent);

        while let Some(index) = parent {
            depth += 1;

            parent = self.node(index).and_then(Node::parent);
        }

        depth
    }

    pub fn visible(&self) -> Vec<usize> {
        let mut children = vec![Vec::new(); self.nodes.len()];

        let mut roots = Vec::new();

        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(parent) = n.parent {
                children[parent].push(i);
            } else {
                roots.push(i);
            }
        }

        let mut todo: Vec<_> = roots.into_iter().rev().collect();

        let mut visible = Vec::new();

        while let Some(i) = todo.pop() {
            visible.push(i);

            if self.nodes[i].expanded {
                todo.extend(children[i].iter().rev().copied());
            }
        }

        visible
    }

    pub fn toggle(&mut self, index: usize) -> bool {
        if !self.has_children(index) {
            return false;
        }

        let Some(node) = self.nodes.get_mut(index) else {
            return false;
        };

        node.expanded = !node.expanded;

        true
    }

    pub fn page(list: &ListState, rect: [i32; 4]) -> usize {
        let content = list.content_rect(rect);

        let height = list.item_size[1].max(0);

        let available = content[3].saturating_sub(content[1]).max(0);

        if list.item_size[1] < 0 {
            0
        } else if height == 0 {
            available as usize
        } else {
            (available / height) as usize
        }
    }

    pub fn layout(&self, list: &ListState, rect: [i32; 4]) -> Layout {
        let visible = self.visible();

        let content = list.content_rect(rect);

        let height = list.item_size[1].max(0);

        let page = Self::page(list, rect);

        let end = if visible.is_empty() && list.empty_range_reset {
            1
        } else {
            visible.len()
        };

        let first = (list.scroll.max(0) as usize).min(end.saturating_sub(page));

        let columns = list.columns(content).max(1) as usize;

        let rows = if height == 0 {
            Vec::new()
        } else {
            visible
                .iter()
                .enumerate()
                .skip(first)
                .filter_map(|(ordinal, &index)| {
                    let y = (ordinal / columns) as i64 - first as i64;

                    let top = i64::from(content[1]) + y * i64::from(height);

                    let bottom = top + i64::from(height);

                    if top < i64::from(content[1]) || bottom > i64::from(content[3]) {
                        return None;
                    }

                    let right = (i64::from(content[0])
                        + (ordinal % columns + 1) as i64 * i64::from(list.item_size[0]))
                    .min(if columns == 1 {
                        i64::from(content[2])
                    } else {
                        i64::from(i32::MAX)
                    });

                    let left = i64::from(content[0]) + (self.depth(index) - 1) as i64 * 25;

                    let bounded = |x: i64| x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;

                    Some((index, [left, top, right, bottom].map(bounded)))
                })
                .collect()
        };

        let bounded = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);

        let range = [0, bounded(end)];

        let bar = list.scrollbar.rects(
            list.scrollbar_rect(rect),
            range,
            bounded(page),
            bounded(first),
        );

        Layout {
            rows,
            visible,
            first,
            page,
            range,
            bar,
        }
    }

    pub(crate) fn remove_prefix(&mut self, count: usize) {
        let count = count.min(self.nodes.len());

        self.nodes.drain(..count);

        for n in &mut self.nodes {
            n.parent = n.parent.and_then(|p| p.checked_sub(count));
        }

        self.hovered = self.hovered.and_then(|p| p.checked_sub(count));
    }
}

impl ListState {
    pub(crate) fn select_tree_mouse(&mut self, rect: [i32; 4], x: i32, y: i32) -> Option<usize> {
        let tree = self.tree.as_ref()?;

        let layout = tree.layout(self, rect);

        let content = self.content_rect(rect);

        let hit = [
            content[0].saturating_sub(self.spacing),
            content[1].saturating_sub(self.spacing),
            content[2].saturating_add(self.spacing),
            content[3].saturating_add(self.spacing),
        ];

        if layout.visible.is_empty() || x < hit[0] || x >= hit[2] || y < hit[1] || y >= hit[3] {
            return None;
        }

        let visible = layout.visible;

        for &i in &visible {
            if let Some(row) = self.items.get_mut(i) {
                row.selected = false;
            }
        }

        let height = self.item_size[1];

        if height <= 0 {
            return None;
        }

        let ordinal =
            (i64::from(y) - i64::from(content[1])) / i64::from(height) + layout.first as i64;

        if ordinal < layout.first as i64
            || ordinal >= layout.first.saturating_add(layout.page) as i64
        {
            return None;
        }

        let index = *visible.get(usize::try_from(ordinal).ok()?)?;

        self.items.get_mut(index)?.selected = true;

        self.selected = index as i32;

        Some(index)
    }

    pub fn add_tree_item(
        &mut self,
        parent: Option<usize>,
        text: &str,
        color: u32,
    ) -> Option<usize> {
        self.add_tree_item_with(
            parent,
            text,
            ItemOptions {
                color,
                ..Default::default()
            },
        )
    }

    pub fn add_tree_item_with(
        &mut self,
        parent: Option<usize>,
        text: &str,
        options: ItemOptions,
    ) -> Option<usize> {
        let tree = self.tree.as_ref()?;

        if parent.is_some_and(|p| p >= self.items.len() || tree.node(p).is_none())
            || text.encode_utf16().count() > 255
        {
            return None;
        }

        let index = self.add_line(text, options.color);

        self.items[index].payload = i64::from(options.payload);

        self.items[index].background = options.background;

        self.items[index].shadow = formats::ui::pane::ShadowType::None;

        self.items[index].shadow_color = options.shadow_color;

        let tree = self.tree.as_mut()?;

        tree.nodes[index].parent = parent;

        tree.nodes[index].tag = options.tag;

        tree.nodes[index].marker = options.marker;

        tree.nodes[index].font = if parent.is_some() { options.font } else { 0 };

        let end = tree.visible().len().min(i32::MAX as usize) as i32;

        self.scroll = self
            .scroll
            .clamp(0, end.saturating_sub(self.scroll_page).max(0));

        Some(index)
    }

    pub fn remove_tree_item(&mut self, index: usize) -> bool {
        let Some(tree) = self.tree.as_mut() else {
            return false;
        };

        if index >= self.items.len() || index >= tree.nodes.len() {
            return false;
        }

        let mut removed = vec![false; tree.nodes.len()];

        removed[index] = true;

        for (i, n) in tree.nodes.iter().enumerate().skip(index + 1) {
            removed[i] = n.parent.is_some_and(|p| removed[p]);
        }

        let mut next = 0;

        let remap: Vec<_> = removed
            .iter()
            .map(|&remove| {
                if remove {
                    None
                } else {
                    let id = next;

                    next += 1;

                    Some(id)
                }
            })
            .collect();

        let mut i = 0;

        self.items.retain(|_| {
            let keep = remap.get(i).is_some_and(Option::is_some);

            i += 1;

            keep
        });

        let mut i = 0;

        tree.nodes.retain(|_| {
            let keep = remap[i].is_some();

            i += 1;

            keep
        });

        for n in &mut tree.nodes {
            n.parent = n.parent.and_then(|p| remap[p]);
        }

        tree.hovered = tree.hovered.and_then(|p| remap.get(p).copied().flatten());

        let remaining = tree.visible().len().min(i32::MAX as usize) as i32;

        self.empty_range_reset = false;

        self.scroll = self
            .scroll
            .clamp(0, remaining.saturating_sub(self.scroll_page).max(0));

        self.selected = usize::try_from(self.selected)
            .ok()
            .and_then(|p| remap.get(p).copied().flatten())
            .map_or(-1, |p| p as i32);

        self.selection_anchor = usize::try_from(self.selection_anchor)
            .ok()
            .and_then(|p| remap.get(p).copied().flatten())
            .map_or(0, |p| p as i32);

        true
    }

    pub fn clear_tree(&mut self) {
        if let Some(tree) = &mut self.tree {
            tree.nodes.clear();

            tree.hovered = None;

            self.items.clear();

            self.selected = -1;

            self.selection_anchor = 0;

            self.empty_range_reset = true;

            self.scroll = self
                .scroll
                .clamp(0, 1i32.saturating_sub(self.scroll_page).max(0));
        }
    }
}

impl crate::Element {
    pub fn add_measured_tree_item(
        &mut self,
        parent: Option<usize>,
        text: &str,
        color: u32,
        measure: &mut dyn crate::layout::Measure,
    ) -> Option<usize> {
        self.add_measured_tree_item_with(
            parent,
            text,
            ItemOptions {
                color,
                ..Default::default()
            },
            measure,
        )
    }

    pub fn add_measured_tree_item_with(
        &mut self,
        parent: Option<usize>,
        text: &str,
        options: ItemOptions,
        measure: &mut dyn crate::layout::Measure,
    ) -> Option<usize> {
        let state = self.widget.list()?;

        let tree = state.tree.as_ref()?;

        if parent.is_some_and(|p| p >= state.items.len() || tree.node(p).is_none())
            || text.encode_utf16().count() > 255
        {
            return None;
        }

        let first = if parent.is_some() && state.richtext {
            let content = state.content_rect(self.rect());

            crate::rich::wrap(
                text,
                self.layers.first()?.font_index,
                content[2].saturating_sub(content[0]),
                0,
                measure,
            )?
            .lines
            .into_iter()
            .next()
        } else {
            None
        };

        let state = self.widget.list_mut()?;

        let index = state.add_tree_item_with(parent, text, options)?;

        if let Some(first) = first {
            state.items[index].text = first.text();

            state.items[index].rich = Some(first);
        }

        Some(index)
    }

    pub fn add_named_tree_child(
        &mut self,
        parent: &str,
        text: &str,
        color: u32,
        measure: &mut dyn crate::layout::Measure,
    ) -> Option<usize> {
        self.add_named_tree_child_with(
            parent,
            text,
            ItemOptions {
                color,
                ..Default::default()
            },
            measure,
        )
    }

    pub fn add_named_tree_child_with(
        &mut self,
        parent: &str,
        text: &str,
        mut options: ItemOptions,
        measure: &mut dyn crate::layout::Measure,
    ) -> Option<usize> {
        let state = self.widget.list()?;

        let index = state.items.iter().position(|row| row.text == parent)?;

        options.background = 0;

        self.add_measured_tree_item_with(Some(index), text, options, measure)
    }

    pub fn add_tagged_tree_child(
        &mut self,
        parent: i32,
        text: &str,
        options: ItemOptions,
        measure: &mut dyn crate::layout::Measure,
    ) -> Option<usize> {
        let state = self.widget.list()?;

        let tree = state.tree.as_ref()?;

        let index = tree
            .nodes
            .iter()
            .take(state.items.len())
            .position(|node| node.tag == parent)?;

        self.add_measured_tree_item_with(Some(index), text, options, measure)
    }
}
