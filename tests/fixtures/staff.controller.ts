import { Body, Controller, Post } from "@nestjs/common";
import { Audited } from "../../shared/audit/index.js";
import { RequireAction } from "../../shared/rbac/index.js";
import { StaffService } from "./staff.service.js";

// Implements FR-VIS-35 for the salon owner role.
@Controller("staff")
export class StaffController extends BaseController {
  @Inject("LOGGER")
  private readonly logger?: Logger;

  constructor(private readonly service: StaffService) {
    super();
  }

  @Post()
  @RequireAction("staff.manage")
  @Audited({ entityType: "staff_member" })
  async create(@Body() dto: CreateStaffDto) {
    return this.service.create(dto);
  }
}

export function helper(): string {
  return "FR-SEC-21";
}
export const LIMIT = 3;
export interface CreateStaffDto {
  name: string;
}
export type Role = "owner" | "master";
export * from "./staff.types.js";
export { StaffService } from "./staff.service.js";
