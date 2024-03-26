use plotters::prelude::*;

#[allow(clippy::too_many_arguments)]
pub fn plot_bar_chart(
    data: Vec<(i32, i32)>, // (mean, std_dev)
    title: &str,
    x_label: &str,
    y_label: &str,
    color: RGBColor,
    error_bars: bool,
    output_path: &std::path::Path,
) {
    if let Some(directory) = output_path.parent() {
        if !directory.exists() {
            std::fs::create_dir_all(directory).expect("Failed to create directory");
        }
    }
    let (means, std_devs): (Vec<_>, Vec<_>) = data.into_iter().unzip();

    let max_x = means.len();
    let max_y = 1000;

    let root_area = BitMapBackend::new(output_path, (1000, 800)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    fn y_label_fmt(x: &i32) -> String {
        format!("{}", x / 10)
    }

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 80)
        .set_label_area_size(LabelAreaPosition::Bottom, 60)
        .caption(title, ("sans-serif", 35))
        .build_cartesian_2d((0..max_x).into_segmented(), 0..max_y)
        .unwrap();

    ctx.configure_mesh()
        .y_label_formatter(&y_label_fmt)
        .x_desc(x_label)
        .y_desc(y_label)
        .x_label_style(("sans-serif", 25))
        .y_label_style(("sans-serif", 25))
        .draw()
        .unwrap();

    ctx.draw_series((0..).zip(means.iter()).map(|(x, y)| {
        let x0 = SegmentValue::Exact(x);
        let x1 = SegmentValue::Exact(x + 1);
        let mut bar = Rectangle::new([(x0, 0), (x1, *y)], color.filled());
        bar.set_margin(0, 0, 5, 5);
        bar
    }))
    .unwrap();

    if error_bars {
        let len = means.len() as i32;
        let mean_std_dev = means.into_iter().zip(std_devs);
        ctx.draw_series((0..len).zip(mean_std_dev).map(|(x, (m, s))| {
            ErrorBar::new_vertical(
                SegmentValue::CenterOf(x as usize),
                m - s,
                m,
                m + s,
                BLACK.filled(),
                10,
            )
        }))
        .unwrap();
    }
}
