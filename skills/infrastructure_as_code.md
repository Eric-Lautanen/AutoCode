---
name: infrastructure-as-code
description: Use when writing or modifying infrastructure configuration - Terraform, Pulumi, CloudFormation, Ansible, or similar. Load when a task involves provisioning cloud resources, managing infrastructure state, or automating environment setup. Covers IaC principles, Terraform patterns, state management, and safe provisioning practices.
---

# Infrastructure as Code

## Overview

Infrastructure as Code (IaC) means defining your servers, networks, databases, and cloud resources in version-controlled files rather than clicking through a console. IaC makes infrastructure reproducible, auditable, and reviewable. The core principles: be declarative (say what you want, not how to get there), be idempotent (running twice produces the same result), and version everything. This skill focuses on Terraform as the most common IaC tool, but the principles apply to Pulumi, CloudFormation, Ansible, and others.

## IaC Principles

1. **Declarative over imperative**: Describe the desired end state. Let the tool figure out how to get there. Avoid scripts that say "create this if it doesn't exist" — the tool should handle that.
2. **Idempotent**: Running `apply` on an already-applied config should produce no changes. This is what makes IaC safe to re-run.
3. **Version controlled**: Every change goes through git. Review infrastructure changes like you review code. `terraform plan` output in the PR.
4. **No manual changes**: If someone clicks a button in the console, your IaC is now out of sync. Detect drift, fix it by importing or re-applying, not by manual adjustment.

## Terraform Basics

### Key Concepts

- **Providers**: Plugins that talk to cloud APIs (aws, gcp, azurerm, kubernetes, etc.)
- **Resources**: Individual infrastructure objects (aws_instance, aws_s3_bucket, etc.)
- **Variables**: Input parameters with types, defaults, and descriptions
- **Outputs**: Values that other modules or consumers need
- **State**: The mapping between your config and real-world resources

### Resource Structure

```hcl
resource "aws_instance" "web" {
  ami           = var.ami_id
  instance_type = var.instance_type

  tags = {
    Name = "web-server"
    Env  = var.environment
  }
}
```

### Variables and Outputs

```hcl
variable "environment" {
  description = "Deployment environment"
  type        = string
  default     = "dev"
}

output "instance_id" {
  description = "ID of the created instance"
  value       = aws_instance.web.id
}
```

## State Management

State is the most critical and dangerous part of Terraform. It maps config resources to real cloud resources.

### Remote State (Required for Teams)

Never store state locally in a team setting. Use remote state:

```hcl
terraform {
  backend "s3" {
    bucket         = "my-tf-state"
    key            = "infra/terraform.tfstate"
    region         = "us-east-1"
    dynamodb_table = "tf-lock"   # State locking
    encrypt        = true
  }
}
```

- **S3 + DynamoDB lock** (AWS), **GCS** (GCP), **Azure Blob** (Azure) — all support locking
- **Never edit state manually** — use `terraform import` and `terraform state` commands
- **Enable encryption** — state contains sensitive values (IPs, ARNs, sometimes secrets)
- **Lock before write** — DynamoDB locking prevents concurrent applies from corrupting state

### State Commands

- `terraform state list` — see what's tracked
- `terraform state show <addr>` — inspect a specific resource
- `terraform state mv <src> <dest>` — rename without destroying
- `terraform state rm <addr>` — stop tracking (doesn't destroy the real resource)
- `terraform import <addr> <id>` — bring an existing resource under management

## Plan Before Apply

**Always** review the plan before applying. Always.

```bash
terraform plan -out=tfplan
terraform apply tfplan
```

- `plan` shows what will change: create, update, or destroy
- Save the plan to a file to ensure the exact plan is applied (no surprises)
- In CI: post the plan output as a PR comment, require human approval before apply
- Watch for **unexpected destroys** — a rename without `state mv` will destroy+create

## Modules

Extract a module when you have a pattern used more than once:

```hcl
module "web_server" {
  source = "./modules/ec2_instance"

  name          = "web"
  instance_type = "t3.medium"
  environment   = var.environment
}
```

### When to Extract a Module

- You've copy-pasted the same resource pattern 2+ times
- The pattern has 5+ resources that always go together (e.g., ALB + listener + target group)
- Different environments need the same pattern with different inputs

### Module Best Practices

- Define all inputs as typed variables with descriptions
- Provide sensible defaults for optional inputs
- Output everything a consumer might need (IDs, ARNs, URLs)
- Keep modules focused — one responsibility per module
- Version modules with git tags when sharing across repos

## Secrets in IaC

**Never hardcode secrets in Terraform files.** They end up in state and in git.

| Approach | How | When |
|----------|-----|------|
| **Vault / SSM Parameter Store** | Reference in Terraform with `data` sources | Production secrets |
| **Environment variables** | `TF_VAR_db_password` pattern | CI/CD pipelines |
| **Sensitive flag** | `sensitive = true` on variable | Hides from plan output |
| **External secret stores** | Fetch at app startup, not in IaC | When Terraform doesn't need the value |

```hcl
variable "db_password" {
  type      = string
  sensitive = true   # Won't show in plan/apply output
}
```

Remember: **state files contain secret values**. Encrypt the state backend and restrict access.

## Drift Detection

Drift = real infrastructure doesn't match your config. Causes: manual console changes, out-of-band scripts, automatic updates.

- Run `terraform plan` regularly (daily in CI) to detect drift
- `terraform plan -detailed-exitcode` returns 2 when drift is detected
- Fix drift by re-applying (IaC wins) or importing (if the manual change should persist)
- Some drift is expected (auto-scaling, patching) — use `lifecycle` blocks to ignore it:

```hcl
lifecycle {
  ignore_changes = [instance_type]  # Allow auto-scaling to change this
}
```

## Destroying Resources

- `terraform destroy` removes everything in the state — use with extreme caution
- `terraform destroy -target=<addr>` removes one resource — useful for debugging
- Always protect production: use `prevent_destroy` lifecycle:

```hcl
lifecycle {
  prevent_destroy = true
}
```

- In CI: never allow `destroy` against production without manual approval

## Common Patterns

### Multi-Environment Setup

```
environments/
  dev/
    main.tf       # Calls modules with dev vars
    terraform.tfvars
  staging/
    main.tf
    terraform.tfvars
  prod/
    main.tf
    terraform.tfvars
modules/
  networking/
  compute/
  database/
```

### Data Sources for Cross-Stack References

```hcl
data "terraform_remote_state" "networking" {
  backend = "s3"
  config = {
    bucket = "my-tf-state"
    key    = "networking/terraform.tfstate"
  }
}

# Use outputs from the networking stack
subnet_id = data.terraform_remote_state.networking.outputs.subnet_id
```

## Windows-Specific IaC Notes

### Terraform on Windows
```powershell
# Install Terraform on Windows via Chocolatey
choco install terraform

# Or via winget
winget install HashiCorp.Terraform

# Initialize Terraform
terraform init

# Plan with Windows paths
terraform plan -var-file="environments\dev\terraform.tfvars"
```

### Windows VM Provisioning with Terraform
```hcl
resource "azurerm_windows_virtual_machine" "example" {
  name                = "win-vm"
  resource_group_name = azurerm_resource_group.example.name
  location            = azurerm_resource_group.example.location
  size                = "Standard_F2"
  admin_username      = "adminuser"
  adminLTSPassword      = var.admin_password

  network_interface_ids = [azurerm_network_interface.example.id]

  os_disk {
    caching              = "ReadWrite"
    storage_account_type = "Standard_LRS"
  }

  source_image_reference {
    publisher = "MicrosoftWindowsServer"
    offer     = "WindowsServer"
    sku       = "2019-Datacenter"
    version   = "latest"
  }
}
```

### PowerShell for Windows Provisioning
```hcl
resource "azurerm_virtual_machine_extension" "example" {
  name                 = "powershell-provision"
  virtual_machine_id   = azurerm_windows_virtual_machine.example.id
  publisher            = "Microsoft.Compute"
  type                 = "CustomScriptExtension"
  type_handler_version = "1.10"

  settings = jsonencode({
    commandToExecute = "powershell.exe -Command \"Write-Output 'Hello from Terraform'\""
  })
}
```

### Windows Container Support
```hcl
resource "azurerm_container_group" "example" {
  name                = "win-container"
  location            = azurerm_resource_group.example.location
  resource_group_name = azurerm_resource_group.example.name
  os_type             = "Windows"

  container {
    name   = "windows-app"
    image  = "mcr.microsoft.com/windows/servercore:ltsc2019"
    cpu    = "2"
    memory = "4"
  }
}
```

### Ansible on Windows
```yaml
# playbook.yml for Windows
- name: Configure Windows Server
  hosts: windows
  gather_facts: yes
  tasks:
    - name: Install IIS
      win_feature:
        name: Web-Server
        state: present
        include_management_tools: yes

    - name: Ensure firewall allows HTTP
      win_firewall_rule:
        name: HTTP
        enabled: yes
        direction: in
        protocol: tcp
        localport: 80
        action: allow
```

## Checklist

- [ ] State is stored remotely with encryption and locking
- [ ] Every variable has a type and description
- [ ] `terraform plan` is reviewed before every apply
- [ ] No secrets are hardcoded — use `sensitive` vars or secret stores
- [ ] Modules extracted for repeated patterns
- [ ] Drift detection runs regularly in CI
- [ ] Production resources have `prevent_destroy` lifecycle
- [ ] State file access is restricted (contains sensitive data)
- [ ] Windows: PowerShell provisioning scripts tested
- [ ] Windows: VM size and image appropriate for workload
- [ ] Windows: Firewall rules configured correctly
