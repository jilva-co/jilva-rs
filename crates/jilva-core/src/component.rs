//! Components: reusable UI units with framework-owned state
//! (`docs/README.md#runtime`).
//!
//! A [`Component`] is a plain value — its configuration, like a label —
//! placed into a [`View`] with [`View::component`]. The runtime gives every
//! mounted component a state slot, keyed by its identity: its position in
//! the view tree, or an explicit [`Key`]. The component's `update` mutates
//! only that slot; its `view` reads it. Neither touches the terminal.
//!
//! [`ComponentId`] is the older Phase-1 identity handle, kept for callers
//! that already name it.

use std::any::{Any, TypeId};
use std::fmt;

use crate::error::{Error, Result};
use crate::events::{Change, Event, InputEvent, Outcome};
use crate::lifecycle::{Context, Effects, FollowUps};
use crate::style::Theme;
use crate::view::View;

/// A reusable UI unit whose state lives in a framework-owned slot.
///
/// `Self` is the component's *configuration*: compared between frames
/// (`PartialEq`) so an unchanged screen is recognisably unchanged, and
/// cloned (`Clone`) into the runtime. Keep it small — labels, options,
/// limits. Ephemeral state (counts, cursors, scroll offsets) goes in
/// [`Component::State`], created by [`Component::init`] when the component
/// first mounts and dropped (after [`Component::cleanup`]) when it unmounts.
///
/// Key events reach the focused component first, then its ancestor
/// components, then the [`Model`](crate::Model). Return
/// [`Outcome::Ignored`] to let an event continue.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use jilva_core::{
///     App, Component, Context, Driven, Effects, Event, InputEvent, KeyEvent, Model, Outcome,
///     Result, TerminalCapabilities, View,
/// };
///
/// #[derive(Clone, PartialEq)]
/// struct Counter;
///
/// impl Component for Counter {
///     type State = i64;
///     type Message = ();
///
///     fn init(&self) -> Result<i64> {
///         Ok(0)
///     }
///
///     fn update(&self, count: &mut i64, event: Event, _effects: &mut Effects) -> Result<Outcome> {
///         if event.key_text() != Some("+") {
///             return Ok(Outcome::Ignored);
///         }
///         *count += 1;
///         Ok(Outcome::repaint())
///     }
///
///     fn view(&self, count: &i64, _cx: &Context) -> Result<View> {
///         Ok(View::text(&count.to_string()))
///     }
///
///     fn focusable(&self) -> bool {
///         true
///     }
/// }
///
/// struct Screen;
///
/// impl Model for Screen {
///     type Message = ();
///
///     fn update(&mut self, _event: Event, _effects: &mut Effects) -> Result<Outcome> {
///         Ok(Outcome::Ignored)
///     }
///
///     fn view(&self, _cx: &Context) -> Result<View> {
///         Ok(View::component(Counter))
///     }
/// }
///
/// let plus = InputEvent::Key(KeyEvent::decode_legacy_byte(b'+').expect("printable"));
/// let mut app = App::new(Screen);
/// app.start(&TerminalCapabilities::conservative(1), (4, 1), Duration::ZERO)?;
/// let frame = app.handle(plus, Duration::from_millis(1))?;
/// assert!(!frame.is_empty()); // the focused counter consumed `+` and repainted
/// # Ok::<(), jilva_core::Error>(())
/// ```
pub trait Component: Clone + PartialEq + 'static {
    /// Ephemeral state owned by the framework slot for this component.
    type State: 'static;
    /// Follow-up messages this component sends itself (`()` when unused).
    type Message: 'static;

    /// Creates the slot state when the component first mounts.
    ///
    /// # Errors
    ///
    /// An error aborts the frame: nothing mounts, nothing is painted.
    fn init(&self) -> Result<Self::State>;

    /// Applies one event to this component's state.
    ///
    /// # Errors
    ///
    /// As for [`Model::update`](crate::Model::update): the batch is abandoned.
    fn update(
        &self,
        state: &mut Self::State,
        event: Event<Self::Message>,
        effects: &mut Effects<Self::Message>,
    ) -> Result<Outcome>;

    /// Describes this component. Pure; [`Context::is_focused`] tells it
    /// whether to draw a focus ring.
    ///
    /// # Errors
    ///
    /// An error aborts the frame cleanly.
    fn view(&self, state: &Self::State, cx: &Context) -> Result<View>;

    /// Releases what the state holds, exactly once, when the component
    /// unmounts. The default just drops the state.
    fn cleanup(&self, state: Self::State) {
        drop(state);
    }

    /// Whether Tab / Shift+Tab can move focus here. Defaults to `false`.
    fn focusable(&self) -> bool {
        false
    }
}

/// An explicit identity for a view among its siblings.
///
/// Put a key on the outermost view of each item in a dynamic list, usually a
/// domain id: the component state then follows the key when items reorder,
/// instead of staying with the position.
///
/// # Example
///
/// ```rust
/// use jilva_core::Key;
///
/// assert_eq!(Key::from(7u64), Key::from(7usize));
/// assert_ne!(Key::from("a"), Key::from("b"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key(KeyRepr);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum KeyRepr {
    Number(u64),
    Text(String),
}

impl From<u64> for Key {
    fn from(value: u64) -> Self {
        Self(KeyRepr::Number(value))
    }
}

impl From<u32> for Key {
    fn from(value: u32) -> Self {
        Self(KeyRepr::Number(u64::from(value)))
    }
}

impl From<usize> for Key {
    fn from(value: usize) -> Self {
        // `usize` is at most 64 bits on every supported target.
        Self(KeyRepr::Number(value as u64))
    }
}

impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self(KeyRepr::Text(value.to_owned()))
    }
}

impl From<String> for Key {
    fn from(value: String) -> Self {
        Self(KeyRepr::Text(value))
    }
}

/// One step of a component's identity path through the view tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Segment {
    /// The n-th child of a container.
    Child(u32),
    /// A keyed child, wherever it sits among its siblings.
    Key(Key),
    /// A duplicate-keyed child: gets fresh state every frame.
    Stateless(u32),
    /// The root of a component's own view.
    Inside,
}

/// A component's identity: the segments from the root view to it.
pub(crate) type Path = Vec<Segment>;

/// What delivering one event to a component produced.
pub(crate) struct Delivered {
    pub(crate) outcome: Outcome,
    pub(crate) change: Change,
    pub(crate) quit: bool,
    pub(crate) theme: Option<Theme>,
}

/// The object-safe face of every [`Component`], used by the runtime.
pub(crate) trait ErasedComponent {
    fn component_type(&self) -> TypeId;
    fn type_name(&self) -> &'static str;
    fn clone_box(&self) -> Box<dyn ErasedComponent>;
    fn eq_dyn(&self, other: &dyn ErasedComponent) -> bool;
    fn as_any(&self) -> &dyn Any;
    fn focusable(&self) -> bool;
    fn init(&self) -> Result<Box<dyn Any>>;
    fn view(&self, state: &dyn Any, cx: &Context) -> Result<View>;
    fn deliver(
        &self,
        state: &mut dyn Any,
        input: InputEvent,
        cx: &Context,
        budget: &mut FollowUps,
    ) -> Result<Delivered>;
    fn cleanup(&self, state: Box<dyn Any>);
}

fn state_mismatch<C>() -> Error {
    Error::Lifecycle(format!(
        "state slot does not hold the state of {}",
        std::any::type_name::<C>()
    ))
}

impl<C: Component> ErasedComponent for C {
    fn component_type(&self) -> TypeId {
        TypeId::of::<C>()
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<C>()
    }

    fn clone_box(&self) -> Box<dyn ErasedComponent> {
        Box::new(self.clone())
    }

    fn eq_dyn(&self, other: &dyn ErasedComponent) -> bool {
        other.as_any().downcast_ref::<C>() == Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn focusable(&self) -> bool {
        Component::focusable(self)
    }

    fn init(&self) -> Result<Box<dyn Any>> {
        Ok(Box::new(Component::init(self)?))
    }

    fn view(&self, state: &dyn Any, cx: &Context) -> Result<View> {
        let state = state
            .downcast_ref::<C::State>()
            .ok_or_else(state_mismatch::<C>)?;
        Component::view(self, state, cx)
    }

    fn deliver(
        &self,
        state: &mut dyn Any,
        input: InputEvent,
        cx: &Context,
        budget: &mut FollowUps,
    ) -> Result<Delivered> {
        let state = state
            .downcast_mut::<C::State>()
            .ok_or_else(state_mismatch::<C>)?;
        #[cfg(debug_assertions)]
        let before = Component::view(self, state, cx).ok();
        let mut effects = Effects::new();
        let outcome = self.update(state, Event::from(input), &mut effects)?;
        let mut change = outcome.change();
        while let Some(message) = effects.next_message() {
            budget.take()?;
            let follow_up = self.update(state, Event::Message(message), &mut effects)?;
            change = change.max(follow_up.change());
        }
        #[cfg(debug_assertions)]
        if change == Change::None
            && let (Some(before), Ok(after)) = (before, Component::view(self, state, cx))
        {
            debug_assert!(
                before == after,
                "component {} reported no change (Outcome::Ignored or Change::None) but its \
                 view() now describes a different screen; return Outcome::repaint() or \
                 Outcome::layout() instead",
                std::any::type_name::<C>()
            );
        }
        #[cfg(not(debug_assertions))]
        let _ = cx;
        Ok(Delivered {
            outcome,
            change,
            quit: effects.quit_requested(),
            theme: effects.take_theme(),
        })
    }

    fn cleanup(&self, state: Box<dyn Any>) {
        if let Ok(state) = state.downcast::<C::State>() {
            Component::cleanup(self, *state);
        }
    }
}

/// A component placed in a [`View`], with its configuration type-erased.
pub(crate) struct ComponentNode(pub(crate) Box<dyn ErasedComponent>);

impl ComponentNode {
    pub(crate) fn new<C: Component>(component: C) -> Self {
        Self(Box::new(component))
    }
}

impl Clone for ComponentNode {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl PartialEq for ComponentNode {
    /// Same component type and equal configuration.
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_dyn(other.0.as_ref())
    }
}

impl Eq for ComponentNode {}

impl fmt::Debug for ComponentNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Component")
            .field(&self.0.type_name())
            .finish()
    }
}

/// Stable identity handle distinguishing components across frames.
///
/// Two components with equal ids are the "same component, new data";
/// different ids mean different components. The runtime identifies mounted
/// components by their path and [`Key`]; this Phase-1 handle makes no claim
/// beyond uniqueness by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(u64);

impl ComponentId {
    /// Creates an identity handle from a raw value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::ComponentId;
    ///
    /// let id = ComponentId::new(7);
    /// assert_eq!(id.get(), 7);
    /// ```
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the raw identity value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ComponentId {
    /// Formats the raw identity value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "component#{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Label(&'static str);

    impl Component for Label {
        type State = u8;
        type Message = ();

        fn init(&self) -> Result<u8> {
            Ok(0)
        }

        fn update(
            &self,
            _state: &mut u8,
            _event: Event,
            _effects: &mut Effects,
        ) -> Result<Outcome> {
            Ok(Outcome::Ignored)
        }

        fn view(&self, _state: &u8, _cx: &Context) -> Result<View> {
            Ok(View::text(self.0))
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Other;

    impl Component for Other {
        type State = ();
        type Message = ();

        fn init(&self) -> Result<()> {
            Ok(())
        }

        fn update(
            &self,
            _state: &mut (),
            _event: Event,
            _effects: &mut Effects,
        ) -> Result<Outcome> {
            Ok(Outcome::Ignored)
        }

        fn view(&self, _state: &(), _cx: &Context) -> Result<View> {
            Ok(View::empty())
        }
    }

    #[test]
    fn identity_round_trips_and_displays() {
        let id = ComponentId::new(7);
        assert_eq!(id.get(), 7);
        assert_eq!(id.to_string(), "component#7");
        assert_eq!(id, ComponentId::new(7));
        assert_ne!(id, ComponentId::new(8));
    }

    #[test]
    fn nodes_compare_by_type_and_configuration() {
        let a = ComponentNode::new(Label("a"));
        assert_eq!(a, ComponentNode::new(Label("a")));
        assert_ne!(a, ComponentNode::new(Label("b")));
        assert_ne!(a, ComponentNode::new(Other));
        assert_eq!(a.clone(), a);
        assert!(format!("{a:?}").contains("Label"));
    }

    #[test]
    fn keys_convert_from_numbers_and_text() {
        assert_eq!(Key::from(3u32), Key::from(3u64));
        assert_eq!(Key::from("id"), Key::from("id".to_string()));
        assert_ne!(Key::from(1u64), Key::from("1"));
    }

    #[test]
    fn a_mismatched_state_slot_is_an_error_not_a_panic() {
        let erased: Box<dyn ErasedComponent> = Box::new(Label("x"));
        let wrong: Box<dyn Any> = Box::new("not a u8");
        let cx = Context::new(
            (4, 1),
            crate::TerminalCapabilities::conservative(0),
            Default::default(),
        );
        assert!(matches!(
            erased.view(wrong.as_ref(), &cx),
            Err(Error::Lifecycle(_))
        ));
        erased.cleanup(wrong); // ignored, never panics
    }
}
