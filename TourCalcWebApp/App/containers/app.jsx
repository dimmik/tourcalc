import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import { BrowserRouter as Router, Route, Switch, Redirect  } from 'react-router-dom';
import LoginScreen from './login-screen.jsx';
import AuthenticatedApp from './authenticated-app.jsx'
import TourUI from './tour-ui.jsx'

export default class App extends React.Component {
    constructor(props) {
        super(props);
        this.state = {}
    }
    render() {
        return (
            <Router>
                <div>
                    <main>
                    <Switch>
                            <Route path="/access/:scope/:code"
                                render={(props) => (<AccessCode app={this} scope={props.match.params.scope} code={props.match.params.code} />)} />
                            <Route path="/tour/:tourid"
                                render={(props) => (<TourUI app={this} tourid={props.match.params.tourid} />)} />
                            <Route path="/" component={Index} />
                    </Switch>
                    </main>
                    </div>
            </Router>
            )
    }
    
};
class AccessCode extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            redirect: false
        }
    }
    
    componentDidMount() {
        AppState.login(this.props.app, this.props.scope, this.props.code)
            .then(() => { this.setState({redirect: true}) })
    }

    render() {
        if (this.state.redirect) {
            return <Redirect to="/"/>
        } else {
            return <div>Checking Access Code</div>
        }
    }
}
class Index extends React.Component {
    constructor(props) {
        super(props);
        this.state = {}
    }
    componentDidMount() {
        AppState.checkWhoAmI(this);
    }

    render() {
        if (!this.state.isAuthLoaded) {
            return (<div>Checking Who you are...</div>)
        } else {
            if (this.state.authData.type == "None") {
                return (<div><pre>{JSON.stringify(this.state.authData, null, 2)}</pre><LoginScreen app={this} /></div>)
            } else {
                return (
                    <div><pre>{JSON.stringify(this.state.authData, null, 2)}</pre><AuthenticatedApp app={this} authData={this.state.authData} /></div>
                )
            }
        }
    }

}
ReactDOM.render(
    <App />,
    document.getElementById("content")
);