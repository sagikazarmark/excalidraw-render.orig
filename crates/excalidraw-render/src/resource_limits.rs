use crate::{ErrorCode, RequestError};

macro_rules! define_resource_limits {
    ($($field:ident: $type:ty = $default:expr, $maximum:expr, $validation:ident;)+) => {
        /// Validated semantic resource ceilings for one render.
        #[derive(Clone, Debug, PartialEq)]
        pub struct ResourceLimits {
            $($field: $type,)+
        }

        const DEFAULT_LIMITS: ResourceLimits = ResourceLimits {
            $($field: $default,)+
        };

        const ABSOLUTE_MAXIMA: ResourceLimits = ResourceLimits {
            $($field: $maximum,)+
        };

        impl ResourceLimits {
            /// Starts a builder from the revision-1 default profile.
            pub fn builder() -> ResourceLimitsBuilder {
                ResourceLimitsBuilder::default()
            }

            /// Returns the immutable absolute-maximum profile.
            pub const fn absolute_maxima() -> &'static Self {
                &ABSOLUTE_MAXIMA
            }

            $(
                #[doc = concat!("Returns the configured `", stringify!($field), "` ceiling.")]
                pub const fn $field(&self) -> $type {
                    self.$field
                }
            )+
        }

        impl Default for ResourceLimits {
            fn default() -> Self {
                DEFAULT_LIMITS.clone()
            }
        }

        /// Builder for validated semantic resource ceilings.
        #[derive(Clone, Debug, Default)]
        pub struct ResourceLimitsBuilder {
            limits: ResourceLimits,
        }

        impl ResourceLimitsBuilder {
            $(
                #[doc = concat!("Sets the `", stringify!($field), "` ceiling.")]
                pub fn $field(mut self, value: $type) -> Self {
                    self.limits.$field = value;
                    self
                }
            )+

            fn validate_individual_limits(&self) -> Result<(), RequestError> {
                macro_rules! validate_limit {
                    (integer, $name:ident) => {
                        if self.limits.$name == 0
                            || self.limits.$name > ABSOLUTE_MAXIMA.$name
                        {
                            return Err(invalid_limit(stringify!($name)));
                        }
                    };
                    (finite, $name:ident) => {
                        if !self.limits.$name.is_finite()
                            || self.limits.$name <= 0.0
                            || self.limits.$name > ABSOLUTE_MAXIMA.$name
                        {
                            return Err(invalid_limit(stringify!($name)));
                        }
                    };
                }

                $(validate_limit!($validation, $field);)+
                Ok(())
            }
        }
    };
}

define_resource_limits! {
    max_scene_bytes: u64 = 32 * 1_048_576, 64 * 1_048_576, integer;
    max_elements: u32 = 16_384, 65_536, integer;
    max_asset_entries: u32 = 1_024, 4_096, integer;
    max_asset_bytes: u64 = 32 * 1_048_576, 64 * 1_048_576, integer;
    max_total_asset_bytes: u64 = 128 * 1_048_576, 256 * 1_048_576, integer;
    max_relationship_depth: u32 = 64, 128, integer;
    max_nested_asset_depth: u32 = 4, 8, integer;
    max_nested_svg_nodes: u32 = 65_536, 262_144, integer;
    max_geometry_segments_per_element: u32 = 262_144, 1_048_576, integer;
    max_total_geometry_segments: u64 = 4_194_304, 16_777_216, integer;
    max_text_scalars_per_element: u32 = 262_144, 1_048_576, integer;
    max_total_text_scalars: u64 = 4_194_304, 16_777_216, integer;
    max_raster_dimension: u32 = 16_384, 32_768, integer;
    max_decoded_pixels_per_asset: u64 = 16_777_216, 67_108_864, integer;
    max_total_decoded_pixels: u64 = 33_554_432, 134_217_728, integer;
    max_raster_surface_pixels: u64 = 33_554_432, 134_217_728, integer;
    max_total_rasterized_pixels: u64 = 67_108_864, 268_435_456, integer;
    max_svg_dimension: f64 = 1_000_000.0, 10_000_000.0, finite;
    max_pdf_page_points: f64 = 14_400.0, 14_400.0, finite;
    max_coordinate_magnitude: f64 = 1e9, 1e12, finite;
    max_diagnostics: u32 = 65_536, 262_144, integer;
    max_working_bytes: u64 = 256 * 1_048_576, 512 * 1_048_576, integer;
    max_output_bytes: u64 = 128 * 1_048_576, 512 * 1_048_576, integer;
    max_element_work_units: u64 = 67_108_864, 268_435_456, integer;
    max_asset_work_units: u64 = 134_217_728, 536_870_912, integer;
    max_total_work_units: u64 = 1_073_741_824, 4_294_967_296, integer;
}

impl ResourceLimitsBuilder {
    /// Validates and constructs the immutable resource-limit profile.
    pub fn build(self) -> Result<ResourceLimits, RequestError> {
        self.validate_individual_limits()?;

        let inconsistent_field = if self.limits.max_asset_bytes > self.limits.max_total_asset_bytes
        {
            Some("max_asset_bytes")
        } else if self.limits.max_geometry_segments_per_element as u64
            > self.limits.max_total_geometry_segments
        {
            Some("max_geometry_segments_per_element")
        } else if self.limits.max_text_scalars_per_element as u64
            > self.limits.max_total_text_scalars
        {
            Some("max_text_scalars_per_element")
        } else if self.limits.max_decoded_pixels_per_asset > self.limits.max_total_decoded_pixels {
            Some("max_decoded_pixels_per_asset")
        } else if self.limits.max_raster_surface_pixels > self.limits.max_total_rasterized_pixels {
            Some("max_raster_surface_pixels")
        } else if self.limits.max_element_work_units > self.limits.max_total_work_units {
            Some("max_element_work_units")
        } else if self.limits.max_asset_work_units > self.limits.max_total_work_units {
            Some("max_asset_work_units")
        } else {
            None
        };

        if let Some(field) = inconsistent_field {
            return Err(invalid_limit(field));
        }

        Ok(self.limits)
    }
}

fn invalid_limit(field: &'static str) -> RequestError {
    RequestError::new(
        ErrorCode::REQUEST_INVALID_RESOURCE_LIMIT,
        "semantic resource limit is invalid",
        field,
    )
}
