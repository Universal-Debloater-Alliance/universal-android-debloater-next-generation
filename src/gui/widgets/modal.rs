use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Alignment, Color, Element, Event, Length, Point, Rectangle, Size, Vector, advanced};

/// A widget that centers a modal element over some base element
pub struct Modal<'a, Message, Theme, Renderer> {
    base: Element<'a, Message, Theme, Renderer>,
    modal: Element<'a, Message, Theme, Renderer>,
    on_blur: Option<Message>,
}

impl<'a, Message, Theme, Renderer> Modal<'a, Message, Theme, Renderer> {
    /// Returns a new [`Modal`]
    pub fn new(
        base: impl Into<Element<'a, Message, Theme, Renderer>>,
        modal: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            base: base.into(),
            modal: modal.into(),
            on_blur: None,
        }
    }

    /// Sets the message that will be produces when the background
    /// of the [`Modal`] is pressed
    pub fn on_blur(self, on_blur: Message) -> Self {
        Self {
            on_blur: Some(on_blur),
            ..self
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Modal<'_, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
    Message: Clone,
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.base), Tree::new(&self.modal)]
    }

    fn size(&self) -> Size<Length> {
        self.base.as_widget().size()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.base, &self.modal]);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.base
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor_position: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.base.as_widget_mut().update(
            &mut state.children[0],
            event,
            layout,
            cursor_position,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.base.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &Renderer,
        viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        Some(overlay::Element::new(Box::new(Overlay {
            position: layout.position(),
            content: &mut self.modal,
            tree: &mut state.children[1],
            size: layout.bounds().size(),
            on_blur: self.on_blur.clone(),
            viewport: *viewport,
        })))
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.base.as_widget().mouse_interaction(
            &state.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        state: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.base
            .as_widget_mut()
            .operate(&mut state.children[0], layout, renderer, operation);
    }
}

struct Overlay<'a, 'b, Message, Theme, Renderer> {
    position: Point,
    content: &'b mut Element<'a, Message, Theme, Renderer>,
    tree: &'b mut Tree,
    size: Size,
    on_blur: Option<Message>,
    viewport: Rectangle,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
    Message: Clone,
{
    fn layout(&mut self, renderer: &Renderer, _bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, self.size)
            .width(Length::Fill)
            .height(Length::Fill);

        let child = self
            .content
            .as_widget_mut()
            .layout(self.tree, renderer, &limits)
            .align(Alignment::Center, Alignment::Center, limits.max());

        layout::Node::with_children(self.size, vec![child]).move_to(self.position)
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Some(message) = self.on_blur.as_ref()
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            )
            && let Some(cursor_position) = cursor.position()
        {
            let content_bounds = layout
                .children()
                .next()
                .expect("Layout must have at least 1 child")
                .bounds();
            if !content_bounds.contains(cursor_position) {
                shell.publish(message.clone());
                return;
            }
        }

        self.content.as_widget_mut().update(
            self.tree,
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            &self.viewport,
        );
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                ..renderer::Quad::default()
            },
            Color {
                a: 0.80,
                ..Color::BLACK
            },
        );

        self.content.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            &layout.bounds(),
        );
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content.as_widget_mut().operate(
            self.tree,
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            self.tree,
            layout.children().next().unwrap(),
            cursor,
            &self.viewport,
            renderer,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Modal<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Theme: 'a,
    Renderer: 'a + advanced::Renderer,
    Message: 'a + Clone,
{
    fn from(modal: Modal<'a, Message, Theme, Renderer>) -> Self {
        Element::new(modal)
    }
}
