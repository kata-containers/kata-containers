#!/usr/bin/env python
# Generate Go map of distros supported by Packagecloud API
# By generating the list once, we save an expensive API call.
# See https://packagecloud.io/docs/api#resource_distributions

import os, sys
import urllib
import json

var = sys.argv[1]
token = os.environ['PACKAGECLOUD_TOKEN']

url = 'https://%s:@packagecloud.io/api/v1/distributions.json' % token
resp = urllib.urlopen(url)
data = json.loads(resp.read())

result = {}
for distros in data.values():
    for d in distros:
        for v in d['versions']:
            k = d['index_name']
            if 'index_name' in v:
                k = '/'.join([k, v['index_name']])
            v = v['id']
            result[k] = v

print '// Generated with %s' % __file__
print
print 'package pkgcloud'
print
print 'var %s = map[string]int{' % var
for k, v in sorted(result.items(),key=lambda x:x[1]):
    print "\t\"%s\": %d," % (k, v)
print '}'
