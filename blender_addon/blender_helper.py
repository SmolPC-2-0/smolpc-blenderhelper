import bpy
import requests
import json
import re
from textwrap import indent
from typing import Optional, Dict, Any

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
        # 1) Try QuickBuilder first for robustness on common shapes/edits
        try:
            if QuickBuilder.try_handle(goal):
                self.report({'INFO'}, "QuickBuilder executed goal")
                try:
                    requests.post(
                        "http://127.0.0.1:17890/blender/remember",
                        json={"event": f"QuickBuilder executed: '{goal}'"},
                        timeout=2,
                    )
                except Exception:
                    pass
                return {'FINISHED'}
        except Exception as qb_err:
            # Fall back to LLM path on any QuickBuilder error
            print(f"QuickBuilder error: {qb_err}")

        # 2) Fallback: call server LLM and execute sanitized code
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
            code = self._normalize_shape_code(goal, code)
            code = self._fix_python(code)
            code = self._fix_blender_ops(code)
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
                # Notify server memory about executed goal (best-effort)
                try:
                    requests.post(
                        "http://127.0.0.1:17890/blender/remember",
                        json={"event": f"Executed macro for goal: '{goal}'"},
                        timeout=2,
                    )
                except Exception:
                    pass
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
            # Skip operator registration lines (not needed for scripts)
            if "bl_idname" in s or "bpy.utils.register_class" in s or ("class " in s and "bpy.types" in s):
                continue
            # Preserve imports and inline code comments - only skip standalone comment lines at the start
            # This allows documentation within complex code
            if s.startswith(("#", "//")) and not any(keyword in s.lower() for keyword in ["import", "from", "todo", "note:", "warning:"]):
                # Skip only if it's a leading comment before any code
                if not any("import" in l or "bpy." in l for l in lines):
                    continue
            lines.append(line)
        cleaned = "\n".join(lines)
        # Only remove leading markdown comments, not inline documentation
        cleaned = re.sub(r"^# [A-Z].*\n", "", cleaned).strip()
        return cleaned

    @staticmethod
    def _detect_shape(goal: str) -> Optional[str]:
        g = (goal or "").lower()
        if "pyramid" in g:
            return "pyramid"
        if "diamond" in g:
            return "diamond"
        if "cone" in g:
            return "cone"
        if "cylinder" in g:
            return "cylinder"
        if "uv sphere" in g:
            return "uv_sphere"
        if "sphere" in g:
            return "uv_sphere"
        if "ico sphere" in g or "icosphere" in g:
            return "ico_sphere"
        if "circle" in g:
            return "circle"
        if "rectangle" in g or "rect" in g or "square" in g or "plane" in g:
            return "plane"
        if "cube" in g or "box" in g:
            return "cube"
        return None

    @classmethod
    def _normalize_shape_code(cls, goal: str, code: str) -> str:
        """Coerce LLM code to the intended primitive if the goal clearly names one.
        - Repairs misnamed operators (object.*_add -> mesh.primitive_*_add)
        - Replaces any *_add line with a canonical line for the target shape
        """
        intent = cls._detect_shape(goal)
        if not intent:
            return code

        canonical_map = {
            "cube": "bpy.ops.mesh.primitive_cube_add(size=2, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
            "cylinder": "bpy.ops.mesh.primitive_cylinder_add(radius=1, depth=2, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
            "cone": "bpy.ops.mesh.primitive_cone_add(radius1=1, depth=2, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
            "pyramid": "bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1, depth=2, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
            "uv_sphere": "bpy.ops.mesh.primitive_uv_sphere_add(radius=1, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
            "ico_sphere": "bpy.ops.mesh.primitive_ico_sphere_add(radius=1, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
            "circle": "bpy.ops.mesh.primitive_circle_add(radius=1, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
            "plane": "bpy.ops.mesh.primitive_plane_add(size=2, enter_editmode=False, align='WORLD', location=(0, 0, 0))",
        }

        canonical_line = canonical_map.get(intent)
        # diamond -> two opposing 4-vert cones
        if intent == "diamond":
            canonical_line = (
                "bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1, depth=1, enter_editmode=False, align='WORLD', location=(0, 0, 0.5))\n"
                "bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1, depth=1, enter_editmode=False, align='WORLD', location=(0, 0, -0.5))\n"
                "bpy.ops.transform.rotate(value=3.14159265, orient_axis='X')\n"
            )
        if not canonical_line:
            return code

        # Remove any existing add-primitive lines to avoid duplicates or wrong args
        filtered_lines = []
        add_call_re = re.compile(r"bpy\.ops\.(?:object|mesh)\.[a-zA-Z_]+_add\s*\(.*\)")
        for ln in code.splitlines():
            if add_call_re.search(ln):
                continue
            filtered_lines.append(ln)

        # Prepend the canonical primitive creation
        new_code = canonical_line + ("\n" if not canonical_line.endswith("\n") else "") + "\n".join(filtered_lines)

        # Ensure we have a handle to the active object after creation
        new_code += "\nobj = bpy.context.active_object\nif obj is not None:\n    bpy.context.view_layer.objects.active = obj\n    obj.select_set(True)\n"

        # Repair a few known misnamed operators that may appear later in code
        repairs = {
            r"bpy\.ops\.object\.cylinder_add": "bpy.ops.mesh.primitive_cylinder_add",
            r"bpy\.ops\.object\.cone_add": "bpy.ops.mesh.primitive_cone_add",
            r"bpy\.ops\.object\.cube_add": "bpy.ops.mesh.primitive_cube_add",
            r"bpy\.ops\.mesh\.cylinder_add": "bpy.ops.mesh.primitive_cylinder_add",
            r"bpy\.ops\.mesh\.cone_add": "bpy.ops.mesh.primitive_cone_add",
            r"bpy\.ops\.mesh\.cube_add": "bpy.ops.mesh.primitive_cube_add",
        }
        for pat, repl in repairs.items():
            new_code = re.sub(pat, repl, new_code)
        return new_code

    @staticmethod
    def _fix_python(code: str) -> str:
        """Lightweight structural repairs to avoid syntax errors from partial blocks.
        - Add 'pass' after bare try:/except:/finally:
        - Replace empty function headers with no-op function and strip its body
        """
        lines = code.splitlines()
        out = []
        i = 0
        while i < len(lines):
            ln = lines[i]
            # Bare blocks that require an indented suite; tolerate trailing comments
            if re.match(r"^\s*try:\s*(?:#.*)?$", ln):
                out.append(ln)
                out.append("    pass")
                i += 1
                continue
            if re.match(r"^\s*except[^:]*:\s*(?:#.*)?$", ln):
                out.append(ln)
                out.append("    pass")
                i += 1
                continue
            if re.match(r"^\s*finally:\s*(?:#.*)?$", ln):
                out.append(ln)
                out.append("    pass")
                i += 1
                continue
            if re.match(r"^\s*(if|elif|else|for|while|with)\b.*:\s*(?:#.*)?$", ln):
                out.append(ln)
                out.append("    pass")
                i += 1
                continue
            # Remove function definitions and their indented bodies
            m = re.match(r"^(\s*)def\s+\w+\s*\(.*\)\s*:\s*$", ln)
            if m:
                base_indent = len(m.group(1))
                # Skip the function body
                i += 1
                while i < len(lines):
                    nxt = lines[i]
                    # line is more indented -> still inside function
                    indent_len = len(nxt) - len(nxt.lstrip(' '))
                    if nxt.strip() == "":
                        i += 1
                        continue
                    if indent_len > base_indent:
                        i += 1
                        continue
                    break
                # Insert a no-op to avoid leaving a blank area
                out.append("pass")
                continue
            out.append(ln)
            i += 1
        # Final sweep: collapse multiple blank lines
        fixed = re.sub(r"\n{3,}", "\n\n", "\n".join(out)).strip() + "\n"
        return fixed

    @staticmethod
    def _fix_blender_ops(code: str) -> str:
        """Normalize common incorrect bpy usage returned by LLMs.
        - Replace active_object.delete() or obj.delete() with ops deletion
        - Map non-existent ops (mesh.boolean_add) to valid modifier calls
        - Gracefully handle generic 'mesh.extrude(' calls to a safe extrude
        - Fix common modifier API mistakes
        - Preserve bmesh and mathutils imports/operations
        """
        lines = code.splitlines()
        out = []
        for ln in lines:
            # Fix object.delete() calls
            m = re.match(r"^(\s*)bpy\.context\.active_object\.delete\s*\(\s*\)\s*$", ln)
            if m:
                ind = m.group(1)
                out.append(ind + "obj = bpy.context.active_object")
                out.append(ind + "if obj:")
                out.append(ind + "    obj.select_set(True)")
                out.append(ind + "    bpy.ops.object.delete(use_global=False)")
                continue
            m2 = re.match(r"^(\s*)obj\.delete\s*\(\s*\)\s*$", ln)
            if m2:
                ind = m2.group(1)
                out.append(ind + "try:")
                out.append(ind + "    obj.select_set(True)")
                out.append(ind + "    bpy.ops.object.delete(use_global=False)")
                out.append(ind + "except Exception:")
                out.append(ind + "    pass")
                continue

            # Normalize bpy.ops.object.delete(...) with unsupported args
            if re.match(r"^\s*bpy\.ops\.object\.delete\s*\(.*\)\s*$", ln):
                ind = re.match(r"^(\s*)", ln).group(1)
                out.append(ind + "bpy.ops.object.delete(use_global=False)")
                continue

            # Replace non-existent mesh.boolean_add with valid boolean modifier
            if ("bpy.ops.mesh.boolean_add" in ln) or ("bpy.ops.object.boolean_add" in ln):
                ind = re.match(r"^(\s*)", ln).group(1)
                args_m = re.search(r"\((.*)\)", ln)
                args = args_m.group(1) if args_m else ""
                op = 'UNION'
                if re.search(r"operation\s*=\s*['\"]DIFFERENCE['\"]", args, re.IGNORECASE):
                    op = 'DIFFERENCE'
                elif re.search(r"operation\s*=\s*['\"]INTERSECT['\"]", args, re.IGNORECASE):
                    op = 'INTERSECT'
                out.append(ind + "try:")
                out.append(ind + "    bpy.ops.object.modifier_add(type='BOOLEAN')")
                out.append(ind + "    _obj = bpy.context.active_object")
                out.append(ind + "    _mod = _obj.modifiers[-1]")
                out.append(ind + f"    _mod.operation = '{op}'")
                out.append(ind + "except Exception:")
                out.append(ind + "    pass")
                continue

            # Fix generic extrude() calls
            if re.match(r"^\s*bpy\.ops\.mesh\.extrude\s*\(.*\)\s*$", ln):
                ind = re.match(r"^(\s*)", ln).group(1)
                out.append(ind + "try:")
                out.append(ind + "    bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={\"value\": (0, 0, 0.1)})")
                out.append(ind + "except Exception:")
                out.append(ind + "    pass")
                continue

            # Fix incorrect modifier.apply() calls - should use bpy.ops.object.modifier_apply()
            if re.match(r"^\s*\w+\.modifiers\[.*\]\.apply\s*\(.*\)\s*$", ln) or re.match(r"^\s*\w+_mod\.apply\s*\(.*\)\s*$", ln):
                ind = re.match(r"^(\s*)", ln).group(1)
                # Extract modifier name/variable
                mod_match = re.search(r'modifiers\["([^"]+)"\]|modifiers\[\'([^\']+)\'\]|(\w+_mod)', ln)
                if mod_match:
                    mod_name = mod_match.group(1) or mod_match.group(2)
                    if mod_name:
                        out.append(ind + f'bpy.ops.object.modifier_apply(modifier="{mod_name}")')
                    else:
                        # If using variable, need to get the name
                        out.append(ind + "# Note: modifier.apply() doesn't exist, use bpy.ops.object.modifier_apply(modifier=name)")
                        out.append(ln)
                else:
                    out.append(ln)
                continue

            # Fix object.*_add calls that should be mesh.primitive_*_add
            if re.match(r"^\s*bpy\.ops\.object\.(cube|sphere|cylinder|cone|torus|plane|circle|grid|uv_sphere|ico_sphere)_add", ln):
                ln = ln.replace("bpy.ops.object.", "bpy.ops.mesh.primitive_")
                # Ensure _add suffix
                for prim in ["cube", "sphere", "cylinder", "cone", "torus", "plane", "circle", "grid"]:
                    ln = ln.replace(f"primitive_{prim}(", f"primitive_{prim}_add(")
                ln = ln.replace("primitive_uv_sphere_add", "primitive_uv_sphere_add")
                ln = ln.replace("primitive_ico_sphere_add", "primitive_ico_sphere_add")

            # Fix size= parameter for unsupported primitives
            if "bpy.ops.mesh.primitive_" in ln and "size=" in ln and not any(k in ln for k in ["primitive_cube_add", "primitive_plane_add"]):
                ind = re.match(r"^(\s*)", ln).group(1)
                size_match = re.search(r"size\s*=\s*([^,)]*)", ln)
                size_expr = size_match.group(1).strip() if size_match else "1.0"
                cleaned = re.sub(r",?\s*size\s*=\s*[^,)]*", "", ln)
                cleaned = cleaned.replace("(,", "(")
                out.append(cleaned)
                out.append(ind + "_obj = bpy.context.active_object")
                out.append(ind + "if _obj:")
                out.append(ind + f"    _obj.scale = ({size_expr}, {size_expr}, {size_expr})")
                continue

            # Fix pyramid (use 4-vertex cone)
            if "bpy.ops.mesh.primitive_pyramid_add" in ln:
                ind = re.match(r"^(\s*)", ln).group(1)
                out.append(ind + "bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1, depth=2, enter_editmode=False, align='WORLD', location=(0, 0, 0))")
                continue

            # Fix incorrect modifier_apply calls (wrong parameter names)
            if "bpy.ops.object.modifier_apply" in ln and "apply_as=" in ln:
                ln = ln.replace("apply_as=", "modifier=")

            # Fix incorrect subdivision surface syntax
            if "bpy.ops.object.subdivision_set" in ln:
                # This operator exists but often misused
                ind = re.match(r"^(\s*)", ln).group(1)
                level_match = re.search(r"level\s*=\s*(\d+)", ln)
                level = level_match.group(1) if level_match else "2"
                out.append(ind + "try:")
                out.append(ind + f"    bpy.ops.object.subdivision_set(level={level})")
                out.append(ind + "except Exception:")
                out.append(ind + "    pass")
                continue

            # Fix direct vertex modification (read-only)
            if re.search(r"\.data\.vertices\[\d+\]\.co\s*=", ln):
                ind = re.match(r"^(\s*)", ln).group(1)
                out.append(ind + "# Warning: vertices are read-only; use bmesh for vertex manipulation")
                out.append(ind + "# " + ln.strip())
                continue

            # Skip problematic scene attributes
            if re.search(r"bpy\.context\.scene\.(unit_system|unit_settings)", ln):
                continue
            if re.search(r"bpy\.data\.modifiers", ln):
                continue

            # Preserve imports (critical for complex operations)
            if ln.strip().startswith("import ") or ln.strip().startswith("from "):
                out.append(ln)
                continue

            # Fix common bmesh mistakes - ensure proper cleanup
            if "bm.to_mesh" in ln:
                out.append(ln)
                # Check if next lines have bm.free(), if not suggest it
                continue

            # Fix modifiers[-1] access (risky if collection is empty)
            if re.search(r"\.modifiers\[-1\]", ln) and "try:" not in ln:
                ind = re.match(r"^(\s*)", ln).group(1)
                # Wrap in safe access
                out.append(ind + "try:")
                out.append(ind + "    " + ln.strip())
                out.append(ind + "except (IndexError, AttributeError):")
                out.append(ind + "    pass")
                continue

            # Fix SubsurfModifier.use_bevel (doesn't exist - common LLM mistake)
            if re.search(r"\.use_bevel\s*=", ln) and "modifier" in ln.lower():
                # Remove this line, use_bevel doesn't exist on modifiers
                ind = re.match(r"^(\s*)", ln).group(1)
                out.append(ind + "# Skipped: use_bevel attribute doesn't exist on modifiers")
                continue

            # Fix incorrect object attributes (width, height, depth on Object)
            # Objects don't have width/height/depth - those are on dimensions
            if re.search(r"\w+\.(width|height|depth)\s*=", ln) and "obj" in ln:
                ind = re.match(r"^(\s*)", ln).group(1)
                attr_match = re.search(r"\.(\w+)\s*=\s*(.+)", ln)
                if attr_match:
                    attr = attr_match.group(1)
                    value = attr_match.group(2).strip()
                    # Map to dimensions
                    dim_map = {"width": "x", "height": "z", "depth": "y"}
                    if attr in dim_map:
                        dim = dim_map[attr]
                        out.append(ind + f"# Fixed: Object doesn't have .{attr}, using .dimensions.{dim}")
                        obj_var = re.search(r"(\w+)\." + attr, ln)
                        if obj_var:
                            var = obj_var.group(1)
                            out.append(ind + f"{var}.dimensions.{dim} = {value}")
                        else:
                            out.append(ln)
                        continue

            # Fix accessing attributes on potentially None modifiers
            # Pattern: mod.levels = X or subsurf.levels = X
            if re.search(r"(\w+_mod|\w+_modifier|subsurf|mirror|array)\.\w+\s*=", ln):
                var_match = re.search(r"((\w+_mod|\w+_modifier|subsurf|mirror|array)\.)", ln)
                if var_match and "if " not in ln and "try:" not in ln:
                    var_name = var_match.group(2)
                    ind = re.match(r"^(\s*)", ln).group(1)
                    # Add None check with try/except
                    out.append(ind + "try:")
                    out.append(ind + f"    if {var_name}:")
                    out.append(ind + "        " + ln.strip())
                    out.append(ind + "except (AttributeError, TypeError):")
                    out.append(ind + "    pass")
                    continue

            # Fix .levels access on modifiers (should check if modifier exists first)
            if re.search(r"\.levels\s*=\s*\d+", ln) and "if " not in ln:
                ind = re.match(r"^(\s*)", ln).group(1)
                out.append(ind + "try:")
                out.append(ind + "    " + ln.strip())
                out.append(ind + "except AttributeError:")
                out.append(ind + "    pass  # Modifier might not support levels")
                continue

            # Fix unsafe object access by name (bpy.data.objects["name"])
            # Convert to safe .get() method
            if re.search(r'bpy\.data\.objects\["[^"]+"\]', ln) or re.search(r"bpy\.data\.objects\['[^']+'\]", ln):
                ind = re.match(r"^(\s*)", ln).group(1)
                # Extract object name
                name_match = re.search(r'bpy\.data\.objects\[(["\'])([^"\']+)\1\]', ln)
                if name_match:
                    obj_name = name_match.group(2)
                    # Check if it's an assignment target
                    if re.match(r"^\s*\w+\s*=", ln):
                        # It's a variable assignment, make it safe
                        var_match = re.match(r"^(\s*)(\w+)\s*=\s*bpy\.data\.objects", ln)
                        if var_match:
                            var_name = var_match.group(2)
                            out.append(ind + f'{var_name} = bpy.data.objects.get("{obj_name}")')
                            out.append(ind + f'if not {var_name}:')
                            out.append(ind + f'    print("Warning: Object {obj_name} not found")')
                            continue
                # For other cases, wrap in try/except
                out.append(ind + "try:")
                out.append(ind + "    " + ln.strip())
                out.append(ind + "except KeyError:")
                out.append(ind + "    pass  # Object not found")
                continue

            # Fix unsafe mesh access by name
            if re.search(r'bpy\.data\.meshes\["[^"]+"\]', ln) or re.search(r"bpy\.data\.meshes\['[^']+'\]", ln):
                ind = re.match(r"^(\s*)", ln).group(1)
                name_match = re.search(r'bpy\.data\.meshes\[(["\'])([^"\']+)\1\]', ln)
                if name_match:
                    mesh_name = name_match.group(2)
                    if re.match(r"^\s*\w+\s*=", ln):
                        var_match = re.match(r"^(\s*)(\w+)\s*=\s*bpy\.data\.meshes", ln)
                        if var_match:
                            var_name = var_match.group(2)
                            out.append(ind + f'{var_name} = bpy.data.meshes.get("{mesh_name}")')
                            continue
                out.append(ind + "try:")
                out.append(ind + "    " + ln.strip())
                out.append(ind + "except KeyError:")
                out.append(ind + "    pass  # Mesh not found")
                continue

            # Fix edit mode operations that need mode switching
            # Common edit mode ops: subdivide, inset, bevel, extrude, loopcut, etc.
            edit_mode_ops = [
                r'bpy\.ops\.mesh\.subdivide\(',
                r'bpy\.ops\.mesh\.inset\(',
                r'bpy\.ops\.mesh\.bevel\(',
                r'bpy\.ops\.mesh\.extrude_region',
                r'bpy\.ops\.mesh\.loopcut',
                r'bpy\.ops\.mesh\.select_',
                r'bpy\.ops\.mesh\.delete\(',
                r'bpy\.ops\.mesh\.merge\(',
                r'bpy\.ops\.mesh\.separate\(',
                r'bpy\.ops\.mesh\.knife_',
            ]
            needs_edit_mode = any(re.search(pattern, ln) for pattern in edit_mode_ops)

            if needs_edit_mode and "bpy.ops.object.mode_set" not in ln:
                # Check if we're already handling mode switching in surrounding context
                ind = re.match(r"^(\s*)", ln).group(1)
                # Wrap in mode switching
                out.append(ind + "# Ensure edit mode for mesh operation")
                out.append(ind + "if bpy.context.active_object and bpy.context.active_object.type == 'MESH':")
                out.append(ind + "    if bpy.context.object.mode != 'EDIT':")
                out.append(ind + "        bpy.ops.object.mode_set(mode='EDIT')")
                out.append(ind + "    " + ln.strip())
                out.append(ind + "    bpy.ops.object.mode_set(mode='OBJECT')")
                continue

            # Fix unsafe .data access on potentially None objects
            # Pattern: obj.data.vertices, obj.data.edges, obj.data.polygons, etc.
            if re.search(r'\w+\.data\.(vertices|edges|polygons|materials|uv_layers)', ln):
                obj_match = re.search(r'(\w+)\.data\.(vertices|edges|polygons|materials|uv_layers)', ln)
                if obj_match and "if " not in ln and "try:" not in ln:
                    obj_var = obj_match.group(1)
                    ind = re.match(r"^(\s*)", ln).group(1)
                    # Use try/except for safety
                    out.append(ind + "try:")
                    out.append(ind + f"    if {obj_var} and hasattr({obj_var}, 'data') and {obj_var}.data:")
                    out.append(ind + "        " + ln.strip())
                    out.append(ind + "except (AttributeError, TypeError):")
                    out.append(ind + "    pass")
                    continue

            # Fix unsafe bpy.context.active_object.data access
            if "bpy.context.active_object.data" in ln and "if " not in ln and "try:" not in ln:
                ind = re.match(r"^(\s*)", ln).group(1)
                out.append(ind + "try:")
                out.append(ind + "    if bpy.context.active_object and bpy.context.active_object.data:")
                out.append(ind + "        " + ln.strip())
                out.append(ind + "except (AttributeError, TypeError):")
                out.append(ind + "    pass")
                continue

            # Fix mesh assignment that might be None
            # Pattern: obj.data = some_mesh (where some_mesh might be None)
            if re.search(r'\w+\.data\s*=\s*\w+', ln) and "bpy.data.meshes" not in ln and "if " not in ln:
                ind = re.match(r"^(\s*)", ln).group(1)
                obj_match = re.search(r'(\w+)\.data\s*=\s*(\w+)', ln)
                if obj_match:
                    obj_var = obj_match.group(1)
                    mesh_var = obj_match.group(2)
                    out.append(ind + "try:")
                    out.append(ind + f"    if {obj_var} and {mesh_var}:")
                    out.append(ind + "        " + ln.strip())
                    out.append(ind + "except (AttributeError, TypeError):")
                    out.append(ind + "    pass")
                    continue

            # Fix operations that require mesh type
            # Accessing .data on object that might not be MESH type
            if re.search(r'\w+\.data\s*=\s*bpy\.data\.meshes\.new', ln):
                # This is fine - creating new mesh data
                pass
            elif re.search(r'\w+\.data\b', ln) and "if " not in ln and "=" not in ln and "try:" not in ln:
                # Reading .data, need to ensure it's a mesh object
                obj_match = re.search(r'(\w+)\.data', ln)
                if obj_match and "bpy.context" not in ln:
                    obj_var = obj_match.group(1)
                    ind = re.match(r"^(\s*)", ln).group(1)
                    out.append(ind + "try:")
                    out.append(ind + f"    if {obj_var} and {obj_var}.type == 'MESH' and {obj_var}.data:")
                    out.append(ind + "        " + ln.strip())
                    out.append(ind + "except (AttributeError, TypeError):")
                    out.append(ind + "    pass")
                    continue

            # Fix bmesh.from_mesh calls that might receive None
            if ("bmesh.from_mesh" in ln or "bm.from_mesh" in ln) and "try:" not in ln:
                ind = re.match(r"^(\s*)", ln).group(1)
                # Extract the mesh argument
                mesh_match = re.search(r'from_mesh\(([^)]+)\)', ln)
                if mesh_match:
                    mesh_arg = mesh_match.group(1).strip()
                    out.append(ind + "try:")
                    out.append(ind + f"    if {mesh_arg}:")
                    out.append(ind + "        " + ln.strip())
                    out.append(ind + "except (AttributeError, TypeError, ValueError):")
                    out.append(ind + "    pass")
                    continue

            out.append(ln)
        return "\n".join(out)


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


class QuickBuilder:
    # simple in-session memory of created objects
    STATE: Dict[str, Any] = {
        "last": None,
        "last_by_kind": {},
        "counters": {"Cube":0, "Pyramid":0, "Cone":0, "Cylinder":0, "UVSphere":0, "IcoSphere":0, "Circle":0, "Plane":0, "Diamond":0, "Duck":0}
    }

    @staticmethod
    def try_handle(goal: str) -> bool:
        if not goal:
            return False
        g = goal.lower().strip()
        # Special: low-poly duck builder
        if "duck" in g:
            QuickBuilder._ensure_object_mode()
            QuickBuilder._add_lowpoly_duck()
            return True
        # Specialized multi-part patterns first
        if "house" in g:
            QuickBuilder._ensure_object_mode()
            size = QuickBuilder._extract_number(g) or 4.0
            base = QuickBuilder._add_cube(size)
            roof_h = size * 0.75
            roof = QuickBuilder._add_pyramid(size, roof_h)
            QuickBuilder._move_active_on_top_of_last(g)
            return True

        # Multi-shape creation: "cube and sphere", "cube, cone and cylinder"
        shapes_in_order = QuickBuilder._find_shapes_ordered(g)
        if len(shapes_in_order) >= 2 and ("on top" not in g):
            QuickBuilder._ensure_object_mode()
            spacing = 2.5
            x = 0.0
            for i, kind in enumerate(shapes_in_order):
                QuickBuilder._create_by_kind(kind)
                obj = bpy.context.active_object
                if obj is None:
                    continue
                obj.location.x = x
                x += max(2.0, obj.dimensions.x) + spacing
            return True

        # Deletions / clearing
        if ("delete all" in g) or ("clear scene" in g) or ("remove all" in g and "objects" in g):
            QuickBuilder._ensure_object_mode()
            try:
                bpy.ops.object.select_all(action='SELECT')
                bpy.ops.object.delete(use_global=False)
                return True
            except Exception:
                return False
        if "delete" in g and ("object" in g or "selected" in g or "active" in g):
            QuickBuilder._ensure_object_mode()
            try:
                sel = [o for o in bpy.context.view_layer.objects if o.select_get()]
                if not sel and bpy.context.active_object is not None:
                    obj = bpy.context.active_object
                    obj.select_set(True)
                bpy.ops.object.delete(use_global=False)
                return True
            except Exception:
                return False
        # Transform existing objects BEFORE creating new ones to avoid accidental creation
        if any(k in g for k in ["move", "translate", "scale", "resize", "rotate"]):
            QuickBuilder._ensure_object_mode()
            # Choose target by kind if mentioned; else last/active
            target = None
            if "pyramid" in g:
                target = QuickBuilder._select_target_by_kind("Pyramid")
            elif "cube" in g or "box" in g:
                target = QuickBuilder._select_target_by_kind("Cube")
            elif "cylinder" in g:
                target = QuickBuilder._select_target_by_kind("Cylinder")
            elif "cone" in g:
                target = QuickBuilder._select_target_by_kind("Cone")
            elif "sphere" in g:
                target = QuickBuilder._select_target_by_kind("UVSphere") or QuickBuilder._select_target_by_kind("IcoSphere")
            if target is None:
                target = bpy.context.active_object or QuickBuilder.STATE.get("last")
            if target is None:
                return False
            # Ensure selected/active
            try:
                for o in bpy.context.view_layer.objects:
                    o.select_set(False)
                bpy.context.view_layer.objects.active = target
                target.select_set(True)
            except Exception:
                pass
            # Now apply the transform
            if "move" in g or "translate" in g:
                dz = QuickBuilder._extract_number(g) or 1.0
                if "up" in g: target.location.z += dz
                elif "down" in g: target.location.z -= dz
                elif "left" in g: target.location.x -= dz
                elif "right" in g: target.location.x += dz
                elif "forward" in g or "front" in g: target.location.y += dz
                elif "back" in g or "backward" in g: target.location.y -= dz
                else: target.location.z += dz
                return True
            if "scale" in g or "resize" in g:
                s = QuickBuilder._extract_number(g) or 1.25
                target.scale = (target.scale.x * s, target.scale.y * s, target.scale.z * s)
                return True
            if "rotate" in g:
                ang = (QuickBuilder._extract_number(g) or 90.0) * 3.14159265 / 180.0
                axis = 'Z'
                if ' x' in g or ' x-' in g or ' x ' in g: axis = 'X'
                elif ' y' in g or ' y-' in g or ' y ' in g: axis = 'Y'
                try:
                    bpy.ops.transform.rotate(value=ang, orient_axis=axis)
                except Exception:
                    # fallback: adjust rotation_euler directly
                    if axis == 'X':
                        target.rotation_euler[0] += ang
                    elif axis == 'Y':
                        target.rotation_euler[1] += ang
                    else:
                        target.rotation_euler[2] += ang
                return True

        # Recognize simple requests and execute directly
        if any(k in g for k in ["cube", "pyramid", "cone", "cylinder", "sphere", "uv sphere", "ico sphere", "circle", "plane", "rectangle", "square", "diamond"]):
            QuickBuilder._ensure_object_mode()
            created = False
            if "diamond" in g:
                obj = QuickBuilder._add_diamond()
                created = obj is not None
            elif "pyramid" in g:
                base = QuickBuilder._extract_number_after(g, ["base", "size", "width"]) or QuickBuilder._same_as_last_size(g) or 2.0
                height = QuickBuilder._extract_number_after(g, ["height"]) or base
                obj = QuickBuilder._add_pyramid(base, height)
                created = obj is not None
            elif "cone" in g:
                radius = QuickBuilder._extract_number(g) or 1.0
                depth = QuickBuilder._extract_second_number(g) or 2.0
                obj = QuickBuilder._add_cone(radius, depth)
                created = obj is not None
            elif "cylinder" in g:
                radius = QuickBuilder._extract_number(g) or 1.0
                depth = QuickBuilder._extract_second_number(g) or 2.0
                obj = QuickBuilder._add_cylinder(radius, depth)
                created = obj is not None
            elif "uv sphere" in g or ("sphere" in g and "ico" not in g):
                radius = QuickBuilder._extract_number(g) or 1.0
                obj = QuickBuilder._add_uv_sphere(radius)
                created = obj is not None
            elif "ico sphere" in g or "icosphere" in g:
                radius = QuickBuilder._extract_number(g) or 1.0
                obj = QuickBuilder._add_ico_sphere(radius)
                created = obj is not None
            elif "circle" in g:
                radius = QuickBuilder._extract_number(g) or 1.0
                obj = QuickBuilder._add_circle(radius)
                created = obj is not None
            elif any(k in g for k in ["plane", "rectangle", "square"]):
                size = QuickBuilder._extract_number(g) or 2.0
                obj = QuickBuilder._add_plane(size)
                created = obj is not None
            elif "cube" in g or "box" in g:
                size = QuickBuilder._parse_cube_size(g) or QuickBuilder._same_as_last_size(g) or 2.0
                obj = QuickBuilder._add_cube(size)
                created = obj is not None

            if created:
                QuickBuilder._post_create_adjustments(g)
                # place on top of previous if requested
                if "on top of" in g or "on top" in g:
                    QuickBuilder._move_active_on_top_of_last(g)
                return True

        # Move/scale/rotate existing active object
        if any(k in g for k in ["move", "translate", "scale", "resize", "rotate"]):
            QuickBuilder._ensure_object_mode()
            obj = bpy.context.active_object
            if obj is None:
                # Try to pick a mesh if any
                mesh_objs = [o for o in bpy.context.view_layer.objects if o.type == 'MESH']
                if mesh_objs:
                    bpy.context.view_layer.objects.active = mesh_objs[0]
                    mesh_objs[0].select_set(True)
                    obj = mesh_objs[0]
            if obj is None:
                return False
            if "move" in g or "translate" in g:
                dz = QuickBuilder._extract_number(g) or 1.0
                if "up" in g: obj.location.z += dz
                elif "down" in g: obj.location.z -= dz
                elif "left" in g: obj.location.x -= dz
                elif "right" in g: obj.location.x += dz
                elif "forward" in g or "front" in g: obj.location.y += dz
                elif "back" in g or "backward" in g: obj.location.y -= dz
                else: obj.location.z += dz
                return True
            if "scale" in g or "resize" in g:
                s = QuickBuilder._extract_number(g) or 1.25
                obj.scale = (obj.scale.x * s, obj.scale.y * s, obj.scale.z * s)
                return True
            if "rotate" in g:
                ang = (QuickBuilder._extract_number(g) or 90.0) * 3.14159265 / 180.0
                axis = 'Z'
                if 'x' in g: axis = 'X'
                elif 'y' in g: axis = 'Y'
                bpy.ops.transform.rotate(value=ang, orient_axis=axis)
                return True

        # Simple composition: "X on top of Y"
        if "on top of" in g and any(shape in g for shape in ["cube","cylinder","cone","pyramid","sphere","uv sphere","ico sphere"]):
            QuickBuilder._ensure_object_mode()
            # naive stack: create base then top and move top up by 1 unit
            parts = g.split("on top of")
            top, base = parts[0], parts[1]
            QuickBuilder.try_handle(base)
            QuickBuilder.try_handle(top)
            QuickBuilder._move_active_on_top_of_last(g)
            return True

        return False

    @staticmethod
    def _ensure_object_mode():
        try:
            if bpy.context.mode != 'OBJECT':
                bpy.ops.object.mode_set(mode='OBJECT')
        except Exception:
            pass

    @staticmethod
    def _parse_cube_size(text: str):
        m = re.search(r"(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)", text)
        if m:
            # use average edge as size
            a, b, c = float(m.group(1)), float(m.group(2)), float(m.group(3))
            return (a + b + c) / 3.0
        m2 = re.search(r"size\s*(\d+(?:\.\d+)?)", text)
        if m2:
            return float(m2.group(1))
        return None

    @staticmethod
    def _extract_number(text: str):
        m = re.search(r"(-?\d+(?:\.\d+)?)", text)
        return float(m.group(1)) if m else None

    @staticmethod
    def _extract_second_number(text: str):
        m = re.findall(r"-?\d+(?:\.\d+)?", text)
        if len(m) >= 2:
            return float(m[1])
        return None

    @staticmethod
    def _extract_number_after(text: str, keys):
        for k in keys:
            m = re.search(rf"{k}[^0-9-]*(-?\d+(?:\.\d+)?)", text)
            if m:
                return float(m.group(1))
        return None

    @staticmethod
    def _same_as_last_size(text: str):
        if "same size" not in text:
            return None
        last = QuickBuilder.STATE.get("last")
        if isinstance(last, bpy.types.Object):
            # use max XY as representative size
            dims = last.dimensions
            return float(max(dims.x, dims.y, dims.z))
        return None

    @staticmethod
    def _post_create_adjustments(text: str):
        # shade smooth or flat
        if "smooth" in text:
            try:
                bpy.ops.object.shade_smooth()
            except Exception:
                pass
        if "flat" in text:
            try:
                bpy.ops.object.shade_flat()
            except Exception:
                pass

    @staticmethod
    def _add_diamond():
        bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1, depth=1, enter_editmode=False, align='WORLD', location=(0, 0, 0.5))
        bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1, depth=1, enter_editmode=False, align='WORLD', location=(0, 0, -0.5))
        bpy.ops.transform.rotate(value=3.14159265, orient_axis='X')
        obj = bpy.context.active_object
        QuickBuilder._remember(obj, "Diamond", {"depth": 2.0})
        return obj

    @staticmethod
    def _add_cube(size: float):
        bpy.ops.mesh.primitive_cube_add(size=size, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "Cube", {"size": size})
        return obj

    @staticmethod
    def _add_plane(size: float):
        bpy.ops.mesh.primitive_plane_add(size=size, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "Plane", {"size": size})
        return obj

    @staticmethod
    def _add_circle(radius: float):
        bpy.ops.mesh.primitive_circle_add(radius=radius, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "Circle", {"radius": radius})
        return obj

    @staticmethod
    def _add_uv_sphere(radius: float):
        bpy.ops.mesh.primitive_uv_sphere_add(radius=radius, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "UVSphere", {"radius": radius})
        return obj

    @staticmethod
    def _add_ico_sphere(radius: float):
        bpy.ops.mesh.primitive_ico_sphere_add(radius=radius, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "IcoSphere", {"radius": radius})
        return obj

    @staticmethod
    def _add_cylinder(radius: float, depth: float):
        bpy.ops.mesh.primitive_cylinder_add(radius=radius, depth=depth, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "Cylinder", {"radius": radius, "depth": depth})
        return obj

    @staticmethod
    def _add_cone(radius1: float, depth: float, vertices: int = 32):
        bpy.ops.mesh.primitive_cone_add(radius1=radius1, depth=depth, vertices=vertices, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "Cone", {"radius1": radius1, "depth": depth, "vertices": vertices})
        return obj

    @staticmethod
    def _add_pyramid(base: float, height: float):
        # Start with a 4-vertex cone; we'll fit to base precisely after creation
        bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=base/2.0, depth=height, enter_editmode=False, align='WORLD', location=(0,0,0))
        obj = bpy.context.active_object
        QuickBuilder._name_and_remember(obj, "Pyramid", {"base": base, "height": height})
        # Fit XY dims to exact 'base' to align edges with cubes/planes regardless of cone's internal radius convention
        try:
            curx, cury = obj.dimensions.x, obj.dimensions.y
            if curx > 0 and cury > 0:
                sx = base / curx
                sy = base / cury
                obj.scale.x *= sx
                obj.scale.y *= sy
        except Exception:
            pass
        return obj

    @staticmethod
    def _move_active_on_top_of_last(text: str):
        last = QuickBuilder.STATE.get("last")
        obj = bpy.context.active_object
        if not obj or not isinstance(last, bpy.types.Object):
            return
        # Compute world-space top of last and move current above it
        last_top = last.location.z + last.dimensions.z / 2.0
        # ensure obj dims up-to-date
        dz = obj.dimensions.z / 2.0
        obj.location.z = last_top + dz
        # Snap XY centers
        obj.location.x = last.location.x
        obj.location.y = last.location.y

    @staticmethod
    def _name_and_remember(obj, kind: str, params: Dict[str, Any]):
        QuickBuilder._remember(obj, kind, params)
        cnt = QuickBuilder.STATE["counters"].get(kind, 0) + 1
        QuickBuilder.STATE["counters"][kind] = cnt
        name = f"{kind}_{cnt}"
        try:
            obj.name = name
        except Exception:
            pass

    @staticmethod
    def _remember(obj, kind: str, params: Dict[str, Any]):
        QuickBuilder.STATE["last"] = obj
        try:
            QuickBuilder.STATE.setdefault("last_by_kind", {})[kind] = obj
        except Exception:
            pass

    @staticmethod
    def _select_target_by_kind(kind: str):
        obj = QuickBuilder.STATE.get("last_by_kind", {}).get(kind)
        if obj is None:
            return None
        try:
            for o in bpy.context.view_layer.objects:
                o.select_set(False)
            bpy.context.view_layer.objects.active = obj
            obj.select_set(True)
        except Exception:
            pass
        return obj

    @staticmethod
    def _add_lowpoly_duck():
        # A simple low-poly duck composed of primitives
        body = QuickBuilder._add_ico_sphere(0.6)
        body.scale = (1.2, 1.8, 1.0)
        head = QuickBuilder._add_ico_sphere(0.3)
        head.location = (0.6, 0.0, 0.35)
        beak = QuickBuilder._add_cone(0.12, 0.25, 16)
        beak.location = (0.9, 0.0, 0.25)
        beak.rotation_euler[1] = 3.14159265 / 2
        tail = QuickBuilder._add_cone(0.15, 0.3, 16)
        tail.location = (-0.9, 0.0, 0.15)
        tail.rotation_euler[1] = -3.14159265 / 2
        eye1 = QuickBuilder._add_uv_sphere(0.05)
        eye1.location = (0.72, 0.12, 0.42)
        eye2 = QuickBuilder._add_uv_sphere(0.05)
        eye2.location = (0.72, -0.12, 0.42)
        try:
            for o in [head, beak, tail, eye1, eye2]:
                o.parent = body
        except Exception:
            pass
        QuickBuilder._name_and_remember(body, "Duck", {})
        return body

    @staticmethod
    def _find_shapes_ordered(text: str):
        t = text
        candidates = {
            "uv sphere": "uv_sphere",
            "ico sphere": "ico_sphere",
            "icosphere": "ico_sphere",
            "pyramid": "pyramid",
            "cone": "cone",
            "cylinder": "cylinder",
            "sphere": "uv_sphere",
            "circle": "circle",
            "plane": "plane",
            "rectangle": "plane",
            "square": "plane",
            "diamond": "diamond",
            "cube": "cube",
            "box": "cube",
        }
        hits = []
        for key, norm in candidates.items():
            idx = t.find(key)
            if idx != -1:
                hits.append((idx, norm))
        hits.sort(key=lambda x: x[0])
        return [norm for _, norm in hits]

    @staticmethod
    def _create_by_kind(kind: str):
        if kind == "cube":
            QuickBuilder._add_cube(2.0)
        elif kind == "plane":
            QuickBuilder._add_plane(2.0)
        elif kind == "uv_sphere":
            QuickBuilder._add_uv_sphere(1.0)
        elif kind == "ico_sphere":
            QuickBuilder._add_ico_sphere(1.0)
        elif kind == "cylinder":
            QuickBuilder._add_cylinder(1.0, 2.0)
        elif kind == "cone":
            QuickBuilder._add_cone(1.0, 2.0, 32)
        elif kind == "pyramid":
            QuickBuilder._add_pyramid(2.0, 2.0)
        elif kind == "diamond":
            QuickBuilder._add_diamond()
        elif kind == "circle":
            QuickBuilder._add_circle(1.0)

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
