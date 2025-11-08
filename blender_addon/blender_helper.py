import bpy
import requests
import json
import re
from textwrap import indent

bl_info = {
    "name": "Blender Helper AI Link",
    "blender": (3, 0, 0),
    "category": "3D View",
}

class BLENDERHELPER_OT_next(bpy.types.Operator):
    bl_idname = "ai.next_step"
    bl_label = "Next Step"

    def execute(self, context):
        goal = context.window_manager.blender_helper_goal
        try:
            res = requests.post(
                "http://127.0.0.1:17890/blender/next_step",
                json={"goal": goal},
                timeout=300,
            )
            res.raise_for_status()
            r = res.json()
            self.report({'INFO'}, r.get("step", "No step returned"))
        except Exception as e:
            self.report({'ERROR'}, f"Next step failed: {e}")
        return {'FINISHED'}


class BLENDERHELPER_OT_do_it(bpy.types.Operator):
    bl_idname = "ai.do_it"
    bl_label = "Do It"

    def execute(self, context):
        goal = context.window_manager.blender_helper_goal
        try:
            res = requests.post(
                "http://127.0.0.1:17890/blender/run_macro",
                json={"goal": goal},
                timeout=300,
            )
            res.raise_for_status()
            r = res.json()
            code = r.get("code", "")
            code = self._extract_code_block(code)
            if not code:
                self.report({'ERROR'}, "No code received from server")
                return {'CANCELLED'}
            # Execute returned bpy Python code in Blender context with safe overrides
            try:
                area = None
                region = None
                win = bpy.context.window
                if win and win.screen:
                    for a in win.screen.areas:
                        if a.type == 'VIEW_3D':
                            area = a
                            break
                    if area:
                        for r in area.regions:
                            if r.type == 'WINDOW':
                                region = r
                                break

                prefix = (
                    "import bpy\n"
                    "try:\n"
                    "    if bpy.context.mode != 'OBJECT':\n"
                    "        bpy.ops.object.mode_set(mode='OBJECT')\n"
                    "except Exception:\n"
                    "    pass\n"
                    "# ensure there is an active object if needed\n"
                    "if bpy.context.active_object is None:\n"
                    "    objs=[o for o in bpy.context.view_layer.objects if o.type=='MESH']\n"
                    "    if objs:\n"
                    "        for o in bpy.context.view_layer.objects:\n"
                    "            o.select_set(False)\n"
                    "        bpy.context.view_layer.objects.active = objs[0]\n"
                    "        objs[0].select_set(True)\n"
                )

                if area and region:
                    wrapped = (
                        prefix
                        + "\nwith bpy.context.temp_override(window=bpy.context.window, area=area, region=region):\n"
                        + indent(code, "    ")
                    )
                    exec(compile(wrapped, "<ai-macro>", "exec"), {"bpy": bpy, "area": area, "region": region})
                else:
                    wrapped = prefix + "\n" + code
                    exec(compile(wrapped, "<ai-macro>", "exec"), {"bpy": bpy})
                self.report({'INFO'}, "Macro executed")
            except Exception as ex:
                self.report({'ERROR'}, f"Macro execution error: {ex}")
                print("--- AI Macro (sanitized) ---\n" + code + "\n--- end ---")
        except Exception as e:
            self.report({'ERROR'}, f"Do It failed: {e}")
        return {'FINISHED'}

    @staticmethod
    def _extract_code_block(text: str) -> str:
        if not text:
            return ""
        # Grab first fenced block marked python/py or any triple-backtick
        m = re.search(r"```(?:python|py)?\s*([\s\S]*?)```", text, re.IGNORECASE)
        if m:
            return m.group(1).strip()
        # If no fences, strip stray backticks and leading markdown headers
        cleaned = text.replace("```", "").replace("`", "")
        # Drop common non-executable lines produced by LLMs
        lines = []
        for line in cleaned.splitlines():
            s = line.strip()
            if not s:
                lines.append(line)
                continue
            if s.startswith(("#", "//")):
                continue
            if "bl_idname" in s or "bpy.utils.register_class" in s or "class " in s and "bpy.types" in s:
                continue
            lines.append(line)
        cleaned = "\n".join(lines)
        cleaned = re.sub(r"^# .*", "", cleaned).strip()
        return cleaned


class BLENDERHELPER_PT_panel(bpy.types.Panel):
    bl_label = "Blender Helper"
    bl_idname = "BLENDERHELPER_PT_panel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'Blender Helper'

    def draw(self, context):
        layout = self.layout
        wm = context.window_manager
        layout.prop(wm, "blender_helper_goal", text="Goal")
        row = layout.row()
        row.operator("ai.next_step", text="Next Step")
        row.operator("ai.do_it", text="Do It")

def register():
    bpy.types.WindowManager.blender_helper_goal = bpy.props.StringProperty(
        name="Goal",
        description="Describe what you want to achieve",
        default="",
    )
    bpy.utils.register_class(BLENDERHELPER_OT_next)
    bpy.utils.register_class(BLENDERHELPER_OT_do_it)
    bpy.utils.register_class(BLENDERHELPER_PT_panel)

def unregister():
    bpy.utils.unregister_class(BLENDERHELPER_PT_panel)
    bpy.utils.unregister_class(BLENDERHELPER_OT_do_it)
    bpy.utils.unregister_class(BLENDERHELPER_OT_next)
    if hasattr(bpy.types.WindowManager, "blender_helper_goal"):
        del bpy.types.WindowManager.blender_helper_goal
