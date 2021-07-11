use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Generate interned strings
    string_cache_codegen::AtomType::new("processor::exts::ExtNameAtom", "ext_name!")
        .atoms(&[
            "GL_3DL_array_objects",
            "GL_AMD_gpu_shader_half_float",
            "GL_AMD_gpu_shader_half_float_fetch",
            "GL_AMD_gpu_shader_int16",
            "GL_AMD_shader_ballot",
            "GL_AMD_shader_fragment_mask",
            "GL_AMD_shader_image_load_store_lod",
            "GL_AMD_texture_gather_bias_lod",
            "GL_ARB_compute_shader",
            "GL_ARB_derivative_control",
            "GL_ARB_enhanced_layouts",
            "GL_ARB_explicit_attrib_location",
            "GL_ARB_explicit_uniform_location",
            "GL_ARB_fragment_shader_interlock",
            "GL_ARB_gpu_shader5",
            "GL_ARB_gpu_shader_fp64",
            "GL_ARB_gpu_shader_int64",
            "GL_ARB_post_depth_coverage",
            "GL_ARB_sample_shading",
            "GL_ARB_separate_shader_objects",
            "GL_ARB_shader_ballot",
            "GL_ARB_shader_bit_encoding",
            "GL_ARB_shader_draw_parameters",
            "GL_ARB_shader_group_vote",
            "GL_ARB_shader_image_load_store",
            "GL_ARB_shader_image_size",
            "GL_ARB_shader_stencil_export",
            "GL_ARB_shader_storage_buffer_object",
            "GL_ARB_shader_texture_image_samples",
            "GL_ARB_shader_texture_lod",
            "GL_ARB_shader_viewport_layer_array",
            "GL_ARB_shading_language_420pack",
            "GL_ARB_shading_language_include",
            "GL_ARB_shading_language_packing",
            "GL_ARB_sparse_texture2",
            "GL_ARB_sparse_texture_clamp",
            "GL_ARB_tessellation_shader",
            "GL_ARB_texture_cube_map_array",
            "GL_ARB_texture_gather",
            "GL_ARB_texture_multisample",
            "GL_ARB_texture_query_lod",
            "GL_ARB_texture_rectangle",
            "GL_ARB_uniform_buffer_object",
            "GL_ARB_vertex_attrib_64bit",
            "GL_ARB_viewport_array",
            "GL_EXT_blend_func_extended",
            "GL_EXT_buffer_reference",
            "GL_EXT_buffer_reference2",
            "GL_EXT_buffer_reference_uvec2",
            "GL_EXT_control_flow_attributes",
            "GL_EXT_debug_printf",
            "GL_EXT_demote_to_helper_invocation",
            "GL_EXT_device_group",
            "GL_EXT_frag_depth",
            "GL_EXT_fragment_invocation_density",
            "GL_EXT_fragment_shading_rate",
            "GL_EXT_geometry_shader",
            "GL_EXT_gpu_shader5",
            "GL_EXT_multiview",
            "GL_EXT_nonuniform_qualifier",
            "GL_EXT_null_initializer",
            "GL_EXT_post_depth_coverage",
            "GL_EXT_primitive_bounding_box",
            "GL_EXT_ray_flags_primitive_culling",
            "GL_EXT_ray_query",
            "GL_EXT_ray_tracing",
            "GL_EXT_samplerless_texture_functions",
            "GL_EXT_scalar_block_layout",
            "GL_EXT_shader_16bit_storage",
            "GL_EXT_shader_8bit_storage",
            "GL_EXT_shader_atomic_float",
            "GL_EXT_shader_atomic_int64",
            "GL_EXT_shader_explicit_arithmetic_types",
            "GL_EXT_shader_explicit_arithmetic_types_float16",
            "GL_EXT_shader_explicit_arithmetic_types_float32",
            "GL_EXT_shader_explicit_arithmetic_types_float64",
            "GL_EXT_shader_explicit_arithmetic_types_int16",
            "GL_EXT_shader_explicit_arithmetic_types_int32",
            "GL_EXT_shader_explicit_arithmetic_types_int64",
            "GL_EXT_shader_explicit_arithmetic_types_int8",
            "GL_EXT_shader_image_int64",
            "GL_EXT_shader_image_load_formatted",
            "GL_EXT_shader_implicit_conversions",
            "GL_EXT_shader_integer_mix",
            "GL_EXT_shader_io_blocks",
            "GL_EXT_shader_non_constant_global_initializers",
            "GL_EXT_shader_subgroup_extended_types_float16",
            "GL_EXT_shader_subgroup_extended_types_int16",
            "GL_EXT_shader_subgroup_extended_types_int64",
            "GL_EXT_shader_subgroup_extended_types_int8",
            "GL_EXT_shader_texture_image_samples",
            "GL_EXT_shader_texture_lod",
            "GL_EXT_shared_memory_block",
            "GL_EXT_terminate_invocation",
            "GL_EXT_tessellation_shader",
            "GL_EXT_texture_buffer",
            "GL_EXT_texture_cube_map_array",
            "GL_EXT_YUV_target",
            "GL_GOOGLE_cpp_style_line_directive",
            "GL_GOOGLE_include_directive",
            "GL_KHR_blend_equation_advanced",
            "GL_KHR_memory_scope_semantics",
            "GL_KHR_shader_subgroup_arithmetic",
            "GL_KHR_shader_subgroup_ballot",
            "GL_KHR_shader_subgroup_basic",
            "GL_KHR_shader_subgroup_clustered",
            "GL_KHR_shader_subgroup_quad",
            "GL_KHR_shader_subgroup_shuffle",
            "GL_KHR_shader_subgroup_shuffle_relative",
            "GL_KHR_shader_subgroup_vote",
            "GL_NV_compute_shader_derivatives",
            "GL_NV_conservative_raster_underestimation",
            "GL_NV_cooperative_matrix",
            "GL_NV_fragment_shader_barycentric",
            "GL_NV_geometry_shader_passthrough",
            "GL_NV_integer_cooperative_matrix",
            "GL_NV_mesh_shader",
            "GL_NV_ray_tracing",
            "GL_NV_sample_mask_override_coverage",
            "GL_NV_shader_atomic_int64",
            "GL_NV_shader_noperspective_interpolation",
            "GL_NV_shader_sm_builtins",
            "GL_NV_shader_subgroup_partitioned",
            "GL_NV_shader_texture_footprint",
            "GL_NV_shading_rate_image",
            "GL_NV_stereo_view_rendering",
            "GL_NV_viewport_array2",
            "GL_NVX_multiview_per_view_attributes",
            "GL_OES_EGL_image_external",
            "GL_OES_EGL_image_external_essl3",
            "GL_OES_geometry_point_size",
            "GL_OES_geometry_shader",
            "GL_OES_gpu_shader5",
            "GL_OES_primitive_bounding_box",
            "GL_OES_sample_variables",
            "GL_OES_shader_image_atomic",
            "GL_OES_shader_io_blocks",
            "GL_OES_shader_multisample_interpolation",
            "GL_OES_standard_derivatives",
            "GL_OES_tessellation_point_size",
            "GL_OES_tessellation_shader",
            "GL_OES_texture_3D",
            "GL_OES_texture_buffer",
            "GL_OES_texture_cube_map_array",
            "GL_OES_texture_storage_multisample_2d_array",
            "GL_OVR_multiview",
        ])
        .write_to_file(&out_dir.join("ext_names.rs"))
        .expect("failed to generate atoms");

    // Generate unit tests from glslangValidator test suite
    tests::generate(&out_dir);
}

mod tests {
    use super::*;
    use heck::SnakeCase;

    const EXCLUDE_PREFIXES: &[&str] = &[
        "hlsl.", "spv.", /* TODO: Remove this when we support attributes */
    ];

    const SHADER_EXTS: &[&str] = &[
        "mesh", "tese", "rgen", "tesc", "geom", "comp", "vert", "frag",
    ];

    pub fn generate(out_dir: &Path) {
        let current_dir = env::current_dir().expect("failed to read current dir");
        let files: Vec<PathBuf> = {
            fs::read_dir(current_dir.join("../glslang/Test"))
                .map(|dir| {
                    dir.into_iter()
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| {
                            entry
                                .file_name()
                                .to_str()
                                .map(|file_name| {
                                    !EXCLUDE_PREFIXES
                                        .iter()
                                        .any(|prefix| file_name.starts_with(prefix))
                                        && SHADER_EXTS.iter().any(|ext| file_name.ends_with(ext))
                                })
                                .unwrap_or(false)
                        })
                        .map(|entry| {
                            entry
                                .path()
                                .strip_prefix(&current_dir)
                                .expect("failed to strip current dir")
                                .to_owned()
                        })
                        .collect()
                })
                .unwrap_or_else(|_| Vec::new())
        };

        let mut f =
            fs::File::create(out_dir.join("glslang_tests.rs")).expect("failed to open output file");

        for test_case in files.into_iter() {
            writeln!(
                f,
                "#[test]
fn test_{test_name}() {{
    common::test_file(r#\"{test_path}\"#);
}}",
                test_name = test_case
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_snake_case()
                    .replace(".", "_")
                    .replace("__", "_"),
                test_path = test_case.to_string_lossy()
            )
            .unwrap();
        }
    }
}
