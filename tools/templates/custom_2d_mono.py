# Godot 4.7.2-stable SCons profile for LWSMF (2D + GLES3 gl_compatibility + .NET/Mono).
#
# Keeps Godot's C++ ClassDB 3D/Viewport bindings intact (disable_3d/physics_3d/navigation_3d
# stay off) so official Godot.NET.Sdk 4.7.2 GodotSharp.dll assemblies initialize Viewport
# without NativeMethodBindNotFoundException, while stripping unused Vulkan/D3D12/Metal,
# AccessKit, XR, 3D authoring modules, and unused networking/media codecs.

target = "template_release"
module_mono_enabled = "yes"
production = "yes"
optimize = "size"
debug_symbols = "no"

# Rendering: GL Compatibility (OpenGL 3 / GLES3) only.
vulkan = "no"
use_volk = "no"
openxr = "no"

# Accessibility: unused screen-reader bridge (AccessKit).
accesskit = "no"

# Unused 3D/XR/networking/media modules.
module_camera_enabled = "no"
module_csg_enabled = "no"
module_dds_enabled = "no"
module_enet_enabled = "no"
module_fbx_enabled = "no"
module_gltf_enabled = "no"
module_gridmap_enabled = "no"
module_hdr_enabled = "no"
module_ktx_enabled = "no"
module_meshoptimizer_enabled = "no"
module_mobile_vr_enabled = "no"
module_multiplayer_enabled = "no"
module_noise_enabled = "no"
module_openxr_enabled = "no"
module_raycast_enabled = "no"
module_theora_enabled = "no"
module_tinyexr_enabled = "no"
module_upnp_enabled = "no"
module_vhacd_enabled = "no"
module_webrtc_enabled = "no"
module_websocket_enabled = "no"
module_webxr_enabled = "no"
module_xatlas_unwrap_enabled = "no"
