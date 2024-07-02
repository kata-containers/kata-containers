# Copyright (c) Microsoft Corporation.
# Adapted from original code by Jeremi Piotrowski <jpiotrowski@microsoft.com>.
from datetime import datetime, timedelta, timezone
import os

from azure.core.exceptions import ResourceNotFoundError
from azure.identity import AzureCliCredential
from azure.mgmt.resource import ResourceManagementClient

# Current date and time in UTC.
utc_now = datetime.now(timezone.utc)
# Cleanup time delta passed in the environment, i.e. how much time to
# wait before automatically cleaning up a resource.
cleanup_after = timedelta(hours=int(os.environ['CLEANUP_AFTER_HOURS']))
# Time considered as the cutoff to clean up a resource: if it was
# created before this time, it will be deleted.
cleanup_cutoff_time = utc_now - cleanup_after

print(f"Current time: {utc_now}")
print(f"Cleanup time delta: {cleanup_after}")
print(f"Will clean up resources created before {cleanup_cutoff_time}")

credential = AzureCliCredential()
subscription_id = os.environ['AZ_SUBSCRIPTION_ID']
client = ResourceManagementClient(credential, subscription_id)
resources = client.resources.list(expand='createdTime')

print("Processsing resources...")
num_deleted = 0

for resource in resources:
    # Ignore resources that aren't AKS clusters.
    if resource.type != 'Microsoft.ContainerService/managedClusters':
        continue

    # Ignore resources created after the cutoff time (i.e. less than
    # `cleanup_after` time ago).
    if resource.created_time > cleanup_cutoff_time:
        print(f"{resource.name}: created at {resource.created_time}, after delta cutoff, ignored")
        continue

    print(f"{resource.name}: created at {resource.created_time}, before delta cutoff, deleting...")

    # A resource ID looks like this:
    # /subscriptions/(subscriptionId)/resourceGroups/(resourceGroupName)/providers/(resourceProviderNamespace)/(resourceType)/(resourceName)
    rg_id, _, _ = resource.id.partition("/providers/")
    _, _, rg_name = rg_id.partition("/resourceGroups/")

    try:
        num_rg_resources = len(list(client.resources.list_by_resource_group(rg_name)))
    except ResourceNotFoundError:
        # Some resource names seem to be lingering in Azure limbo but do
        # not map to any actual resources, so we ignore those.
        print(f"{resource.name}: not found, ignored")
        continue

    # XXX DANGER ZONE: Delete the resource. If it's the only resource
    # in its resource group, the entire resource group is deleted.

    if num_rg_resources == 1:
        client.resource_groups.begin_delete(rg_name)
        print(f"{resource.name}: deleted resource group")
    else:
        client.resources.begin_delete_by_id(resource.id)
        print(f"{resource.name}: deleted resource")

    num_deleted += 1

print(f"Deleted {num_deleted} resources")
