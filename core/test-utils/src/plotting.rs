use std::collections::BTreeMap;

use plotters::prelude::*;
use plotters::style::full_palette::{RED_800, TEAL_600};

#[allow(unused)]
#[allow(clippy::too_many_arguments)]
pub fn plot_bar_chart(
    data: Vec<(i32, i32)>, // (mean, std_dev)
    title: &str,
    x_label: &str,
    y_label: &str,
    color: RGBColor,
    error_bars: bool,
    darkmode: bool,
    output_path: &std::path::Path,
) -> anyhow::Result<()> {
    if let Some(directory) = output_path.parent() {
        if !directory.exists() {
            std::fs::create_dir_all(directory)?;
        }
    }
    let (means, std_devs): (Vec<_>, Vec<_>) = data.into_iter().unzip();

    let (bg_color, primary_color) = if darkmode {
        (BLACK, WHITE)
    } else {
        (WHITE, BLACK)
    };

    let max_x = means.len();
    let max_y = 1000;

    let root_area = BitMapBackend::new(output_path, (1000, 800)).into_drawing_area();
    root_area.fill(&bg_color).unwrap();
    let title_style = TextStyle::from(("sans-serif", 35).into_font()).color(&primary_color);
    let root_area = root_area.titled(title, title_style)?;

    fn y_label_fmt(x: &i32) -> String {
        format!("{}", x / 10)
    }

    let mut ctx = ChartBuilder::on(&root_area)
        .margin(30)
        .set_label_area_size(LabelAreaPosition::Left, 100)
        .set_label_area_size(LabelAreaPosition::Bottom, 80)
        .build_cartesian_2d((0..max_x).into_segmented(), 0..max_y)?;

    ctx.configure_mesh()
        .y_label_formatter(&y_label_fmt)
        .x_desc(x_label)
        .y_desc(y_label)
        .axis_style(ShapeStyle {
            color: primary_color.into(),
            filled: false,
            stroke_width: 1,
        })
        .axis_desc_style(("sans-serif", 25, &primary_color))
        .label_style(("sans-serif", 25, &primary_color))
        .x_label_style(("sans-serif", 25, &primary_color))
        .y_label_style(("sans-serif", 25, &primary_color))
        .bold_line_style(ShapeStyle {
            color: primary_color.into(),
            filled: false,
            stroke_width: 1,
        })
        .draw()?;

    ctx.draw_series((0..).zip(means.iter()).map(|(x, y)| {
        let x0 = SegmentValue::Exact(x);
        let x1 = SegmentValue::Exact(x + 1);
        let mut bar = Rectangle::new([(x0, 0), (x1, *y)], color.filled());
        bar.set_margin(0, 0, 5, 5);
        bar
    }))?;

    if error_bars {
        let len = means.len() as i32;
        let mean_std_dev = means.into_iter().zip(std_devs);
        ctx.draw_series((0..len).zip(mean_std_dev).map(|(x, (m, s))| {
            ErrorBar::new_vertical(
                SegmentValue::CenterOf(x as usize),
                m - s,
                m,
                m + s,
                RED_800.filled(),
                10,
            )
        }))?;
    }
    Ok(())
}

#[allow(unused)]
#[allow(clippy::too_many_arguments)]
pub fn line_plot(
    data: &BTreeMap<usize, BTreeMap<usize, (f64, f64)>>,
    title: &str,
    x_label: &str,
    y_label: &str,
    error_bars: bool,
    darkmode: bool,
    output_path: &std::path::Path,
) -> anyhow::Result<()> {
    if let Some(directory) = output_path.parent() {
        if !directory.exists() {
            std::fs::create_dir_all(directory)?;
        }
    }

    let mut min_x = usize::MAX;
    let mut max_x = usize::MIN;

    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    data.iter().for_each(|(_, m)| {
        m.iter().for_each(|(k, (m, s))| {
            min_x = min_x.min(*k);
            max_x = max_x.max(*k);

            min_y = min_y.min(m - s);
            max_y = max_y.max(m + s);
        });
    });

    let (bg_color, primary_color) = if darkmode {
        (BLACK, WHITE)
    } else {
        (WHITE, BLACK)
    };

    let root_area = BitMapBackend::new(output_path, (1000, 800)).into_drawing_area();
    root_area.fill(&bg_color)?;

    let title_style = TextStyle::from(("sans-serif", 25).into_font()).color(&primary_color);
    let root_area = root_area.titled(title, title_style)?;

    let mut ctx = ChartBuilder::on(&root_area)
        .margin(40)
        .set_label_area_size(LabelAreaPosition::Left, 120)
        .set_label_area_size(LabelAreaPosition::Bottom, 100)
        .build_cartesian_2d(min_x..max_x, min_y..max_y)?;

    ctx.configure_mesh()
        .x_desc(x_label)
        .y_desc(y_label)
        .axis_style(ShapeStyle {
            color: primary_color.into(),
            filled: false,
            stroke_width: 1,
        })
        .axis_desc_style(("sans-serif", 25, &primary_color))
        .label_style(("sans-serif", 25, &primary_color))
        .x_label_style(("sans-serif", 25, &primary_color))
        .y_label_style(("sans-serif", 25, &primary_color))
        .bold_line_style(ShapeStyle {
            color: primary_color.into(),
            filled: false,
            stroke_width: 1,
        })
        .draw()?;

    let color_map = ViridisRGBA {};
    let min_line_index = *data.keys().min().unwrap() as f64;
    let max_line_index = *data.keys().max().unwrap() as f64;
    for (line_index, line_data) in data {
        let color =
            color_map.get_color_normalized(*line_index as f64, min_line_index, max_line_index);
        ctx.draw_series(LineSeries::new(
            line_data.iter().map(|(x, (mean, _))| (*x, *mean)),
            ShapeStyle {
                color,
                filled: false,
                stroke_width: 4,
            },
        ))?
        .label(format!("{line_index}"))
        .legend(move |(x, y)| {
            PathElement::new(
                vec![(x, y), (x + 20, y)],
                ShapeStyle {
                    color,
                    filled: false,
                    stroke_width: 4,
                },
            )
        });

        ctx.draw_series(line_data.iter().map(|(x, (mean, var))| {
            let s = var.sqrt();
            ErrorBar::new_vertical(
                *x,
                mean - s,
                *mean,
                mean + s,
                ShapeStyle {
                    color: TEAL_600.into(),
                    filled: true,
                    stroke_width: 2,
                },
                20,
            )
        }))?;
    }

    ctx.configure_series_labels()
        .label_font(("sans-serif", 20, &bg_color))
        .background_style(ShapeStyle {
            color: primary_color.into(),
            filled: true,
            stroke_width: 2,
        })
        .draw()?;

    Ok(())
}
