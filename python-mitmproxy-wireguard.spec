# Copyright 2022 Wong Hoi Sing Edison <hswong3i@pantarei-design.com>
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

%global debug_package %{nil}

Name: python-mitmproxy-wireguard
Epoch: 100
Version: 0.1.18
Release: 1%{?dist}
Summary: WireGuard frontend for mitmproxy
License: MIT
URL: https://github.com/decathorpe/mitmproxy_wireguard/tags
Source0: %{name}_%{version}.orig.tar.gz
BuildRequires: cargo
BuildRequires: fdupes
BuildRequires: python-rpm-macros
BuildRequires: python3-cython
BuildRequires: python3-devel
BuildRequires: python3-maturin >= 0.13
BuildRequires: python3-pip
BuildRequires: python3-setuptools
BuildRequires: rust >= 1.56.0

%description
Transparently proxy any device that can be configured as a WireGuard
client.

%prep
%autosetup -T -c -n %{name}_%{version}-%{release}
tar -zx -f %{S:0} --strip-components=1 -C .

%build
maturin build --offline --sdist

%install
pip install \
    --root=%{buildroot} \
    --prefix=%{_prefix} \
    target/wheels/*.whl
find %{buildroot}%{python3_sitearch} -type f -name '*.pyc' -exec rm -rf {} \;
fdupes -qnrps %{buildroot}%{python3_sitearch}

%check

%if 0%{?suse_version} > 1500
%package -n python%{python3_version_nodots}-mitmproxy-wireguard
Summary: WireGuard frontend for mitmproxy
Requires: python3
Provides: python3-mitmproxy-wireguard = %{epoch}:%{version}-%{release}
Provides: python3dist(mitmproxy-wireguard) = %{epoch}:%{version}-%{release}
Provides: python%{python3_version}-mitmproxy-wireguard = %{epoch}:%{version}-%{release}
Provides: python%{python3_version}dist(mitmproxy-wireguard) = %{epoch}:%{version}-%{release}
Provides: python%{python3_version_nodots}-mitmproxy-wireguard = %{epoch}:%{version}-%{release}
Provides: python%{python3_version_nodots}dist(mitmproxy-wireguard) = %{epoch}:%{version}-%{release}

%description -n python%{python3_version_nodots}-mitmproxy-wireguard
Transparently proxy any device that can be configured as a WireGuard
client.

%files -n python%{python3_version_nodots}-mitmproxy-wireguard
%license LICENSE
%{python3_sitearch}/*
%endif

%if !(0%{?suse_version} > 1500)
%package -n python3-mitmproxy-wireguard
Summary: WireGuard frontend for mitmproxy
Requires: python3
Provides: python3-mitmproxy-wireguard = %{epoch}:%{version}-%{release}
Provides: python3dist(mitmproxy-wireguard) = %{epoch}:%{version}-%{release}
Provides: python%{python3_version}-mitmproxy-wireguard = %{epoch}:%{version}-%{release}
Provides: python%{python3_version}dist(mitmproxy-wireguard) = %{epoch}:%{version}-%{release}
Provides: python%{python3_version_nodots}-mitmproxy-wireguard = %{epoch}:%{version}-%{release}
Provides: python%{python3_version_nodots}dist(mitmproxy-wireguard) = %{epoch}:%{version}-%{release}

%description -n python3-mitmproxy-wireguard
Transparently proxy any device that can be configured as a WireGuard
client.

%files -n python3-mitmproxy-wireguard
%license LICENSE
%{python3_sitearch}/*
%endif

%changelog
