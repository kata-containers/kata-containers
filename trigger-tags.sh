tag=1.9.0-alpha2
git tag -d "${tag}"
git push egernst HEAD :"${tag}"

git tag -a "${tag}" -m "test push"
git push egernst HEAD "${tag}"
