pub mod mode;
mod palette;

use gtk::gdk::RGBA;
use mode::{MouseMode, MouseModeVariant};
use super::canvas::Canvas;
use super::UiState;
use palette::Palette;
use crate::image::brush::{Brush, BrushType};
use crate::image::blend::BlendingMode;
use crate::image::resize::ScaleMethod;
use crate::ui::icon;
use crate::shape::ShapeType;
use crate::transformable::Transformable;
use super::toolbar::mode::ModeToolbar;

use gtk::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use glib_macros::clone;

pub struct Toolbar {
    widget: gtk::Box,
    mode_button_box: gtk::Box,
    palette_p: Rc<RefCell<Palette>>,
    mouse_mode: MouseMode,
    mouse_mode_buttons: Vec<MouseModeButton>,
    mode_change_hook: Option<Box<dyn Fn(&Toolbar, Option<MouseMode>, MouseMode)>>,
    mode_toolbar: ModeToolbar,
    primary_brush: Brush,
    secondary_brush: Brush,
    /// One-pixel brush to use for the eyedropper,
    /// solely for the visual of highlighting one pixel
    eyedropper_brush: Brush,
    /// Used by self-closing modes (free transform) to recover
    /// the previous mode
    last_two_mode_variants: (MouseModeVariant, MouseModeVariant),
    /// Hack to allow text tool to use non-copyable state
    boxed_transformable: RefCell<Option<Box<dyn Transformable>>>,
    /// Handle to the currently open text dialog window (if any)
    active_text_dialog: RefCell<Option<gtk::Window>>,
}

struct MouseModeButton {
    mode: MouseMode,
    widget: gtk::ToggleButton,
}

const INITIAL_MODE: MouseMode = MouseMode::cursor_default();

impl Toolbar {
    pub fn new_p() -> Rc<RefCell<Toolbar>> {
        let default_primary_color = RGBA::new(0.0, 0.0, 0.0, 1.0);
        let default_secondary_color = RGBA::new(0.0, 0.0, 0.0, 0.0);
        let default_palette_colors = vec![
            vec![
                Some(RGBA::new(0.0, 0.0, 0.0, 1.0)),
                Some(RGBA::new(1.0, 0.0, 0.0, 1.0)),
                Some(RGBA::new(0.0, 1.0, 0.0, 1.0)),
                Some(RGBA::new(0.0, 0.0, 1.0, 1.0)),
                Some(RGBA::new(0.0, 0.0, 0.0, 0.0)),
            ],
            vec![None, None, None, None, None],
        ];

        let widget =  gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let mode_button_box =  gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let palette_p = Palette::new_p(
            default_primary_color, default_secondary_color, default_palette_colors
        );
        let mode_toolbar_wrapper = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .hexpand(true)
            .vexpand(false)
            .build();
        let mode_toolbar = ModeToolbar::new(&mode_toolbar_wrapper, Some(INITIAL_MODE.variant()));
        let primary_brush = Brush::new(default_primary_color, default_secondary_color, BrushType::Round, 5);
        let secondary_brush = Brush::new(default_secondary_color, default_primary_color, BrushType::Round, 5);
        let eyedropper_brush = Brush::new(default_primary_color, default_secondary_color, BrushType::Square, 1);

        widget.append(&mode_button_box);
        // If you think this is over then you're wrong...
        widget.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        widget.append(palette_p.borrow().widget());
        widget.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        widget.append(&mode_toolbar_wrapper);

        let toolbar_p = Rc::new(RefCell::new(Toolbar {
            widget,
            mode_button_box,
            palette_p,
            mouse_mode: INITIAL_MODE,
            mouse_mode_buttons: vec![],
            mode_change_hook: None,
            mode_toolbar,
            primary_brush,
            secondary_brush,
            eyedropper_brush,
            last_two_mode_variants: (MouseModeVariant::Cursor, MouseModeVariant::Cursor),
            boxed_transformable: RefCell::new(None),
            active_text_dialog: RefCell::new(None),
        }));

        toolbar_p
    }

    pub fn init_ui_hooks(ui_p: &Rc<RefCell<UiState>>) {
        let toolbar_p = ui_p.borrow().toolbar_p.clone();

        let button_info: Vec<(_, &str, fn(&mut Canvas) -> MouseMode, fn() -> MouseMode)> = vec![
            (&icon::CURSOR, "Cursor", MouseMode::cursor, MouseMode::cursor_default),
            (&icon::PENCIL, "Pencil", MouseMode::pencil, MouseMode::pencil_default),
            (&icon::EYEDROPPER, "Eyedropper", MouseMode::eyedropper, MouseMode::eyedropper_default),
            (&icon::RECTANGLE_SELECT, "Rectangle Select", MouseMode::rectangle_select, MouseMode::rectangle_select_default),
            (&icon::MAGIC_WAND, "Magic Wand", MouseMode::magic_wand, MouseMode::magic_wand_default),
            (&icon::FILL, "Fill", MouseMode::fill, MouseMode::fill_default),
            (&icon::FREE_TRANSFORM, "Free Transform", MouseMode::free_transform, MouseMode::free_transform_default),
            (&icon::SHAPE, "Shape", MouseMode::shape, MouseMode::shape_default),
            (&icon::TEXT, "Text", MouseMode::text, MouseMode::text_default),
        ];

        toolbar_p.borrow_mut().mouse_mode_buttons = button_info.into_iter()
            .map(|(icon_texture, tooltip, mode_constructor, mode_constructor_default)| {
                let icon_widget = gtk::Image::from_paintable(Some(&**icon_texture));

                let button = gtk::ToggleButton::builder()
                    .child(&icon_widget)
                    .width_request(75)
                    .height_request(75)
                    .tooltip_text(tooltip)
                    .build();

                button.connect_clicked(clone!(@strong toolbar_p, @strong ui_p => move |b| {
                    if b.is_active() {
                        let new_mode =
                            if let Some(canvas_p) = ui_p.borrow().active_canvas_p()  {
                                mode_constructor(&mut canvas_p.borrow_mut())
                            } else {
                                mode_constructor_default()
                            };

                        let old_mode = toolbar_p.borrow_mut().set_mouse_mode(new_mode.clone());

                        if let Some(ref f) = toolbar_p.borrow().mode_change_hook {
                            f(&toolbar_p.borrow(), old_mode, new_mode);
                        }
                    } else {
                        // the only way to deactivate is to activate a different modal button
                        b.set_active(true);
                    }
                }));

                toolbar_p.borrow_mut().mode_button_box.append(&button);

                MouseModeButton {
                    widget: button,
                    mode: mode_constructor_default(),
                }
            })
            .collect::<Vec<_>>();

        // activate INITIAL_MODE button
        toolbar_p.borrow_mut().mouse_mode_buttons.iter().for_each(|b| {
            if b.mode.variant() == INITIAL_MODE.variant() {
                b.widget.set_active(true);
            }
        });

        toolbar_p.borrow_mut().mode_toolbar.init_ui_hooks(ui_p);
    }

    pub fn mouse_mode(&self) -> &MouseMode {
        &self.mouse_mode
    }

    pub fn mouse_mode_mut(&mut self) -> &mut MouseMode {
        &mut self.mouse_mode
    }

    pub fn last_two_mouse_mode_variants(&self) -> (MouseModeVariant, MouseModeVariant) {
        self.last_two_mode_variants
    }

    /// Sets the current mouse mode to `new_mouse_mode`, returning
    /// the old one
    pub fn set_mouse_mode(&mut self, new_mouse_mode: MouseMode) -> Option<MouseMode> {
        let current_variant = self.mouse_mode.variant();
        if self.last_two_mode_variants.1 != current_variant {
            self.last_two_mode_variants = (
                self.last_two_mode_variants.1,
                current_variant
            );
        }

        for b in self.mouse_mode_buttons.iter() {
            b.widget.set_active(b.mode.variant() == new_mouse_mode.variant());
        }

        let old_mode = std::mem::replace(&mut self.mouse_mode, new_mouse_mode.clone());
        self.mode_toolbar.set_to_variant(new_mouse_mode.variant());

        Some(old_mode.clone()).filter(move |_| old_mode.variant() != self.mouse_mode.variant())
    }

    pub fn primary_color(&self) -> RGBA {
        self.palette_p.borrow().primary_color()
    }

    pub fn secondary_color(&self) -> RGBA {
        self.palette_p.borrow().secondary_color()
    }

    pub fn set_primary_color(&self, color: RGBA) {
        self.palette_p.borrow_mut().set_primary_color(color);
    }

    pub fn set_secondary_color(&self, color: RGBA) {
        self.palette_p.borrow_mut().set_secondary_color(color);
    }

    fn add_color_to_palette(&mut self, color: RGBA) -> Result<(), ()> {
        self.palette_p.borrow_mut().add_color(color)
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.widget
    }

    pub fn set_mode_change_hook(&mut self, f: Box<dyn Fn(&Toolbar, Option<MouseMode>, MouseMode)>) {
        self.mode_change_hook = Some(f);
    }

    fn get_primary_brush(&mut self) -> &Brush {
        let primary_color = self.primary_color();
        let secondary_color = self.secondary_color();
        let (brush_type, _blending_mode, radius) = self.mode_toolbar.get_pencil_settings();
        self.primary_brush.modify(primary_color, secondary_color, brush_type, radius);
        &self.primary_brush
    }

    fn get_secondary_brush(&mut self) -> &Brush {
        let primary_color = self.primary_color();
        let secondary_color = self.secondary_color();
        let (brush_type, _blending_mode, radius) = self.mode_toolbar.get_pencil_settings();
        self.secondary_brush.modify(secondary_color, primary_color, brush_type, radius);
        &self.secondary_brush
    }

    fn get_primary_brush_mut(&mut self) -> &mut Brush {
        self.get_primary_brush();
        &mut self.primary_brush
    }

    fn get_eyedropper_brush_mut(&mut self) -> &mut Brush {
        &mut self.eyedropper_brush
    }

    fn get_blending_mode(&self) -> BlendingMode {
        let (_brush_type, blending_mode, _radius) = self.mode_toolbar.get_pencil_settings();
        blending_mode
    }

    fn get_magic_wand_tolerance(&self) -> f64 {
        self.mode_toolbar.get_magic_wand_settings().0
    }

    fn get_fill_tolerance(&self) -> f64 {
        self.mode_toolbar.get_fill_settings().0
    }

    fn get_magic_wand_relativity(&self) -> bool {
        self.mode_toolbar.get_magic_wand_settings().1
    }

    fn get_fill_relativity(&self) -> bool {
        self.mode_toolbar.get_fill_settings().1
    }

    fn get_shape_type(&self) -> ShapeType {
        self.mode_toolbar.get_shape_settings().0.clone()
    }

    fn get_shape_border_width(&self) -> u8 {
        self.mode_toolbar.get_shape_settings().1
    }

    fn get_clamp_translate(&self) -> bool {
        self.mode_toolbar.get_free_transform_settings().0
    }

    fn get_clamp_scale(&self) -> bool {
        self.mode_toolbar.get_free_transform_settings().1
    }

    fn get_clamp_rotate(&self) -> bool {
        self.mode_toolbar.get_free_transform_settings().2
    }

    pub fn get_free_transform_scale_method(&self) -> ScaleMethod {
        self.mode_toolbar.get_free_transform_settings().3
    }

    pub fn set_boxed_transformable(&self, boxed_transformable: Box<dyn Transformable>) {
        *self.boxed_transformable.borrow_mut() = Some(boxed_transformable);
    }

    pub fn get_boxed_transformable(&self) -> std::cell::RefMut<Option<Box<dyn Transformable>>> {
        self.boxed_transformable.borrow_mut()
    }

    pub fn try_take_boxed_transformable(&self) -> Option<Box<dyn Transformable>> {
        std::mem::replace(&mut self.boxed_transformable.borrow_mut(), None)
    }

    pub fn set_active_text_dialog(&self, window: gtk::Window) {
        *self.active_text_dialog.borrow_mut() = Some(window);
    }

    pub fn close_active_text_dialog(&self) {
        if let Some(win) = self.active_text_dialog.borrow_mut().take() {
            win.close();
        }
    }
}
